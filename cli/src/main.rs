mod utils;
use std::{
    collections::HashMap,
    io::{Cursor, Write},
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use clap::{builder::Str, command, CommandFactory, Parser};

use clap_derive::Parser;
use console::{Style, Term};
use dialoguer::MultiSelect;

use dotenv::dotenv;
use futures::stream;
use futures_util::StreamExt;
use human_panic::setup_panic;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle, TermLike};
use tsticker::bot::{Bot, Sticker, StickerSet, TelegramFile};

static STYLE_PROGRESSBAR_LEN: &'static str = "[{elapsed_precise}] {bar} {pos:>7}/{len:7} {msg}";

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(
        short,
        long,
        help = "Telegram bot token, or set TELEGRAM_BOT_TOKEN in environment variable"
    )]
    pub token: Option<String>,
    #[arg(short, long, default_value=std::env::current_dir().unwrap().into_os_string())]
    pub output: PathBuf,
    #[arg(required = true, help = "Sticker links, get it by sharing button")]
    pub links: Vec<String>,

    #[arg(short, long, default_value_t = false)]
    pub fast_failure: bool,
}

async fn build_bot(token: String) -> Bot {
    let mut term = Term::stdout();

    term.write_line("[1/4] Login bot...").unwrap();
    let bot = match Bot::login(token).await {
        Ok(bot) => bot,
        Err(e) => {
            term.write_fmt(format_args!(
                "{}: fail to login, {}\n",
                Style::new().red().apply_to("Error"),
                e.to_string()
            ))
            .unwrap();
            std::process::exit(-1)
        }
    };
    term.write_fmt(format_args!(
        "Hello, {}@{}\n",
        Style::new().green().apply_to(&bot.me().first_name),
        Style::new().blue().apply_to(&bot.me().username)
    ))
    .unwrap();
    bot
}

async fn get_sticker_set(bot: &Bot, links: Vec<String>) -> Vec<StickerSet> {
    let mut term = Term::stdout();

    term.write_line("[2/4] Retrieve sticker set list...")
        .unwrap();
    let links: Vec<String> = links
        .into_iter()
        .map(|x| {
            if x.contains("/") {
                x.split("/").last().unwrap().to_owned()
            } else {
                x
            }
        })
        .collect();

    let mut sticker_set = Vec::new();

    let prog = if let Ok(style) = ProgressStyle::with_template(&STYLE_PROGRESSBAR_LEN) {
        ProgressBar::new(links.len() as u64).with_style(style)
    } else {
        ProgressBar::new(links.len() as u64)
    };

    for name in links {
        prog.set_message(name.clone());
        let set = match bot.request_sticker_set(&name).await {
            Ok(e) => e,
            Err(err) => {
                term.write_fmt(format_args!(
                    "{}: fail to retrieve sticker set of {}, {}",
                    Style::new().red().apply_to("Error"),
                    name,
                    err.to_string()
                ))
                .unwrap();
                std::process::exit(-1)
            }
        };
        prog.inc(1);
        sticker_set.push(set);
    }
    prog.finish_with_message("done");

    sticker_set
}

fn select_sticker_set(items: Vec<StickerSet>) -> Vec<StickerSet> {
    let selection = MultiSelect::new()
        .with_prompt("Please select sticker set you want to download:")
        .items(&items)
        .interact()
        .unwrap();

    let sticker_set: Vec<StickerSet> = items
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| selection.contains(idx))
        .map(|(_, sticker_set)| sticker_set)
        .collect();

    Term::stdout()
        .write_fmt(format_args!(
            "Download sticker set: {}\n",
            sticker_set
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        ))
        .unwrap();
    sticker_set
}

fn convert_sticker_to_filename(sticker: &Sticker) -> String {
    let emoji_name = emojis::get(&sticker.emoji)
        .map(|e| e.name())
        .unwrap_or("emoji_missing")
        .to_string();

    let file_id = &sticker.file_id;

    let ext = sticker.file_ext().to_string();

    format!("{}_{}.{}", emoji_name, file_id, ext)
}

async fn download_sticker_set(
    bot: &Bot,
    sticker_sets: Vec<StickerSet>,
    dest_dir: PathBuf,
    parallel_number: Option<usize>,
    fast_failure: bool,
) {
    let term = Term::stdout();

    term.write_line("[3/4] Downloading sticker...").unwrap();

    let mp = MultiProgress::new();
    let sty = ProgressStyle::with_template(&STYLE_PROGRESSBAR_LEN)
        .unwrap_or(ProgressStyle::default_bar());

    let total_num_sticker = sticker_sets.iter().map(|s| s.stickers.len() as u64).sum();

    // Progressbar of sticker to get item url
    let pb_fetch_url_sticker = mp
        .add(ProgressBar::new(total_num_sticker))
        .with_style(sty.clone());

    // Get file id of each sticker

    let mp_ref = &mp;
    let term_ref = &term;
    let dest_dir_ref = &dest_dir;

    let pb_sticker_set = mp
        .add(ProgressBar::new(sticker_sets.len() as u64))
        .with_style(sty.clone());

    let pb_sticker = mp.add(ProgressBar::new(0)).with_style(sty.clone());

    let pb_sticker_set_ref = &pb_sticker_set;
    let pb_sticker_ref = &pb_sticker;
    stream::iter(sticker_sets)
        .enumerate()
        .then(|(idx, sticker_set)| async move {
            let StickerSet {
                title, stickers, ..
            } = sticker_set;
            pb_sticker_set_ref.set_message(format!("Parsing sticker items of {}", title));
            pb_sticker_set_ref.set_position(idx as u64 +1);

            let stickers = stream::iter(stickers)
                .then(|s| async {
                    let id = bot.request_file_id(&s).await;
                    (s, id)
                })
                .filter_map(|(s, file_id_res)| async move {
                    let mut term = term_ref.clone();
                    match file_id_res {
                        Ok(v) => Some((s, v)),
                        Err(e) => {
                            term.write_fmt(format_args!(
                                "{}: fail to get file url of sticker {}({}), {}",
                                Style::new().red().apply_to("Error"),
                                s.emoji,
                                s.file_id,
                                e.to_string()
                            ))
                            .unwrap();
                            if fast_failure {
                                panic!("fail to get file url of sticker");
                            }
                            None
                        }
                    }
                })
                .collect::<Vec<_>>()
                .await;

            (idx, (title, stickers))
        })
        .for_each(|(idx, (sticker_title, stickers))| async move {
            let title_ref = sticker_title.as_str();
            pb_sticker_set_ref.set_message(format!("Downloading stickers {}", sticker_title));
            pb_sticker_set_ref.set_position(idx as u64 +1);

            pb_sticker_ref.set_position(0);
            pb_sticker_ref.set_length(stickers.len() as u64);
            pb_sticker_ref.set_message(format!("Preparing stickers {}", sticker_title));

            stream::iter(stickers)
                .then(|(sticker, file_id_res)| async move {

                    pb_sticker_ref.inc(1);
                    pb_sticker_ref.set_message(format!("Downloading sticker {}", sticker.emoji));
                    (sticker, bot.download_file(&file_id_res).await)
                })
                .filter_map(|(sticker, resp_res)| async move {
                    let mut term = term_ref.clone();
                    match resp_res {
                        Ok(v) => Some((sticker, v)),
                        Err(e) => {
                            term.write_fmt(format_args!(
                                "{}: fail to get sticker {}({}), {}",
                                Style::new().red().apply_to("Error"),
                                sticker.emoji,
                                sticker.file_id,
                                e.to_string()
                            ))
                            .unwrap();
                            if fast_failure {
                                panic!("fail to get sticker");
                            }
                            None
                        }
                    }
                })
                .filter_map(|(sticker, resp)| async move {
                    let mut term = term_ref.clone();
                    let file_path = convert_sticker_to_filename(&sticker);
                    let mut dst = dest_dir_ref.clone();
                    dst.push(title_ref.clone());
                    dst.push(file_path);

                    // Create destination directory if not exists
                    if let Some(parent) = dst.parent() {
                        if !parent.exists() {
                            let res = tokio::fs::create_dir_all(&parent).await;
                            if let Err(err) = res {
                                term.write_fmt(format_args!(
                                    "{}: {}",
                                    Style::new().red().apply_to("Error"),
                                    err.to_string()
                                ))
                                .unwrap();
                                if fast_failure {
                                    panic!("fail to create destination directory");
                                } else {
                                    return None;
                                }
                            }
                        }
                    }

                    let file = match tokio::fs::File::create(&dst).await {
                        Ok(file) => file,
                        Err(err) => {
                            term.write_fmt(format_args!(
                                "{}: fail to create local file {},  {}",
                                dst.as_path().to_string_lossy(),
                                Style::new().red().apply_to("Error"),
                                err.to_string()
                            ))
                            .unwrap();

                            if fast_failure {
                                panic!("fail to create local file");
                            } else {
                                return None;
                            }
                        }
                    };

                    Some((dst, file, resp))
                })
                .for_each_concurrent(parallel_number, |(dst, mut file, resp)| async move {
                    let mut bytes = resp.bytes_stream();
                    let mut term = term_ref.clone();
                    while let Some(item) = bytes.next().await {
                        match item {
                            Ok(data) => {
                                if let Err(err) =
                                    tokio::io::copy(&mut data.as_ref(), &mut file).await
                                {
                                    term.write_fmt(format_args!(
                                        "{}: fail to write to file {}, {}",
                                        dst.as_path().to_string_lossy(),
                                        Style::new().red().apply_to("Error"),
                                        err.to_string()
                                    ))
                                    .unwrap();
                                    if fast_failure {
                                        std::process::exit(-1);
                                    }
                                };
                            }
                            Err(e) => {
                                term.write_fmt(format_args!(
                                    "{}: fail to download file {}, {}",
                                    dst.as_path().to_string_lossy(),
                                    Style::new().red().apply_to("Error"),
                                    e.to_string()
                                ))
                                .unwrap();
                                if fast_failure {
                                    panic!("fail to fetch bytes")
                                }
                            }
                        }
                    }
                })
                .await;
        })
        .await;
}

#[tokio::main]
async fn main() {
    setup_panic!();
    let Args {
        token,
        output,
        links,
        fast_failure,
    } = Args::parse();
    dotenv().ok();

    let token = if let Some(token) = token {
        token
    } else {
        if let Ok(env) = std::env::var("TELEGRAM_BOT_TOKEN") {
            env
        } else {
            panic!("Token is not set");
        }
    };

    let bot = build_bot(token).await;
    let sticker_set = select_sticker_set(get_sticker_set(&bot, links).await);
    download_sticker_set(&bot, sticker_set, output, None, fast_failure).await;
}
