use std::{
    io::{Cursor, Write},
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use clap::{builder::Str, command, CommandFactory, Parser};

use clap_derive::Parser;
use console::{Style, Term};
use dialoguer::MultiSelect;
use futures_util::stream::StreamExt;
use human_panic::setup_panic;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle, TermLike};
use tsticker::bot::{Bot, Sticker, StickerSet, TelegramFile};

static STYLE_PROGRESSBAR_LEN: &'static str = "[{elapsed_precise}] {bar} {pos:>7}/{len:7} {msg}";

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub token: String,
    #[arg(short, long, default_value=std::env::current_dir().unwrap().into_os_string())]
    pub output: PathBuf,

    pub links: Vec<String>,
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
        let set = match bot.get_sticker_set(&name).await {
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
    let mut filename = emojis::get(&sticker.emoji)
        .map(|e| e.name())
        .unwrap_or("missing")
        .to_string();

        filename.push_str("_");
    filename.push_str(&sticker.file_id);
    match (sticker.is_animated, sticker.is_video) {
        (false, false) => filename.push_str(".webp"),
        (true, false) => filename.push_str(".webm"),
        (false, true) => filename.push_str(".webm"),
        _ => unimplemented!(),
    }
    filename
}

async fn download_sticker_set(
    bot: &Bot,
    sticker_set: Vec<StickerSet>,
    dest_dir: PathBuf,
    parallel_number: Option<i32>,
) {
    let mut term = Term::stdout();

    term.write_line("[3/4] Downloading sticker...").unwrap();

    let mp = MultiProgress::new();
    let sty = ProgressStyle::with_template(&STYLE_PROGRESSBAR_LEN)
        .unwrap_or(ProgressStyle::default_bar());

    let total_num_sticker = sticker_set.iter().map(|s| s.stickers.len() as u64).sum();

    // Sticker to be get item url
    let pb_fetch_url_sticker = mp
        .add(ProgressBar::new(total_num_sticker))
        .with_style(sty.clone());

    let (sticker_tx, sticker_rx) = async_channel::bounded(8);
    let (file_tx, mut file_rx) = tokio::sync::mpsc::unbounded_channel();

    let url_fetcher_bot = bot.clone();
    tokio::spawn(async move {
        for sticker_set in &sticker_set {
            for sticker in &sticker_set.stickers {
                pb_fetch_url_sticker.set_message(format!(
                    "retrieve path {}/{}",
                    sticker_set.title, sticker.emoji
                ));
                match url_fetcher_bot.get_url(sticker).await {
                    Ok(url) => sticker_tx
                        .send((url, sticker.clone(), sticker_set.title.clone()))
                        .await
                        .unwrap(),
                    Err(err) => {
                        term.write_fmt(format_args!(
                            "{}: fail to get url of sticker {}({}), {}",
                            Style::new().red().apply_to("Error"),
                            sticker.emoji,
                            sticker.file_id,
                            err.to_string()
                        ))
                        .unwrap();

                        std::process::exit(-2)
                    }
                };
                pb_fetch_url_sticker.inc(1);
            }
        }
        pb_fetch_url_sticker.finish_with_message("done");
        sticker_tx.close();
    });

    let mut downloader_handle = Vec::new();
    for _ in 0..parallel_number.unwrap_or(16) {
        let dest_dir = dest_dir.clone();
        let bot = bot.clone();
        let rx = sticker_rx.clone();
        let file_tx = file_tx.clone();
        let handle = tokio::spawn(async move {
            let mut term = Term::stdout();
            while let Ok((url, sticker, sticker_set_title)) = rx.recv().await {
                let resp = bot.send_download_request(&url).await;
                let resp = match resp {
                    Ok(resp) => resp,
                    Err(err) => {
                        term.write_fmt(format_args!(
                            "{}: fail to open response, {}",
                            Style::new().red().apply_to("Error"),
                            err
                        ))
                        .unwrap();
                        continue;
                    }
                };

                let mut file_path = dest_dir.clone();
                file_path.push(&sticker_set_title);
                file_path.push(convert_sticker_to_filename(&sticker));
                if let Some(parent) = file_path.parent() {
                    if !parent.exists() {
                        let res = tokio::fs::create_dir_all(&parent).await;
                        if let Err(err) = res {
                            term.write_fmt(format_args!(
                                "{}: {}",
                                Style::new().red().apply_to("Error"),
                                err.to_string()
                            ))
                            .unwrap();
                            continue;
                        }
                    }
                }
                let mut byte_stream = resp.bytes_stream();
                let mut file = match tokio::fs::File::create(&file_path).await {
                    Ok(file) => file,
                    Err(err) => {
                        term.write_fmt(format_args!(
                            "{}: fail to create local file, {}",
                            Style::new().red().apply_to("Error"),
                            err
                        ))
                        .unwrap();
                        continue;
                    }
                };
                let mut err = None;

                while let Some(item) = byte_stream.next().await {
                    if item.is_err() {
                        err = Some(item.unwrap_err().to_string());
                        break;
                    }
                    let item = item.unwrap();
                    if let Err(error) = tokio::io::copy(&mut item.as_ref(), &mut file).await {
                        err = Some(error.to_string());
                        break;
                    }
                }

                if let Some(e) = err {
                    term.write_fmt(format_args!(
                        "{}: fail to download file, {}",
                        Style::new().red().apply_to("Error"),
                        e
                    ))
                    .unwrap();
                    continue;
                }
                file_tx
                    .send((file_path, sticker, sticker_set_title))
                    .unwrap();
            }
        });
        downloader_handle.push(handle);
    }
    std::mem::drop(file_tx);

    let pb_download_sticker = mp
        .add(ProgressBar::new(total_num_sticker))
        .with_style(sty.clone());
    // let mut term = Term::stdout();
    while let Some((path, sticker, sticker_set_title)) = file_rx.recv().await {
        pb_download_sticker.inc(1);
        pb_download_sticker.set_message(format!(
            "download {}/{}",
            &sticker_set_title, &sticker.emoji
        ));
        // term.write_fmt(format_args!("download {}/{} to {}", &sticker_set_title, &sticker.emoji, path.as_os_str().to_string_lossy())).unwrap();
    }
    pb_download_sticker.finish_with_message("completed")
}

#[tokio::main]
async fn main() {
    setup_panic!();
    let Args {
        token,
        output,
        links,
    } = Args::parse();

    let bot = build_bot(token).await;
    let sticker_set = select_sticker_set(get_sticker_set(&bot, links).await);
    download_sticker_set(&bot, sticker_set, output, None).await;
}
