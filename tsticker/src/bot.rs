use std::{rc::Rc, sync::Arc};

use log::{debug, info};
use reqwest::{Client, Response, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    bot,
    error::{Error, Result},
};

#[derive(Debug, Clone)]
pub struct Bot {
    token: String,
    client: Client,
    info: Arc<BotInfo>,
}

#[derive(Debug, serde::Deserialize)]
struct TelegramResp<T> {
    ok: bool,
    result: T,
}

#[derive(Debug, serde::Deserialize)]
struct TelegramStatus {
    ok: bool,
    error_code: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct BotInfo {
    id: i64,
    pub first_name: String,
    pub username: String,
}

async fn request<T>(client: &Client, url: Url) -> Result<T>
where
    T: DeserializeOwned,
{
    let target = url.path().to_owned();
    let resp = client.get(url).send().await?.text().await?;
    debug!("request {}: {}", target, resp);
    if !serde_json::from_str::<TelegramStatus>(&resp)
        .map_err(Error::ResponseJsonError)?
        .ok
    {
        return Err(Error::BotError(resp));
    }

    let ans = serde_json::from_str::<TelegramResp<T>>(&resp)
        .map_err(Error::ResponseJsonError)?
        .result;
    Ok(ans)
}

async fn request_telegram<T>(
    client: &Client,
    token: &str,
    path: &str,
    params: &[(&'static str, &str)],
) -> Result<T>
where
    T: DeserializeOwned,
{
    let mut entry = format!("https://api.telegram.org/bot{}", &token);
    if !path.starts_with('/') {
        entry.push_str("/");
    }
    entry.push_str(path);
    let url = reqwest::Url::parse_with_params(&entry, params)
        .expect("Unexpected error when parse url with params");
    request(client, url).await
}

pub trait TelegramFile : Sync + Send{
    fn file_id(&self)-> &str;
    fn file_size(&self) -> u64;
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ThumbFile {
    file_id: String,
    file_unique_id: String,
    file_size: u64,
    width: u32,
    height: u32
}

impl TelegramFile for ThumbFile {
    fn file_id(&self)-> &str {
        &self.file_id
    }

    fn file_size(&self) -> u64 {
        self.file_size
    }
}


#[derive(Debug, serde::Deserialize, Clone)]
pub struct Sticker{
    pub width: u32,
    pub height: u32,
    pub emoji: String,
    // useless
    // set_name: String, 
    pub is_animated: bool,
    pub is_video: bool,
    #[serde(rename = "type")]
    pub ty: String,
    pub thumbnail: ThumbFile,
    pub thumb: ThumbFile,
    pub file_id: String,
    file_unique_id: String,
    file_size: u64
}

impl TelegramFile for Sticker{
    fn file_id(&self)-> &str {
        &self.file_id
    }

    fn file_size(&self) -> u64 {
        self.file_size
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct StickerSet{
    pub name: String,
    pub title: String,
    pub sticker_type: String,
    pub stickers: Vec<Sticker>
}

impl ToString for StickerSet{
    fn to_string(&self) -> String {
        self.title.clone()
    }
}


#[derive(Deserialize, Debug)]
struct FileInfoResp {
    file_path: String,
    // useless attr
    // file_id: String,
    // file_size: u64,
    // file_unique_id: String,
}

pub struct TelegramFilePath(String);
impl ToString for TelegramFilePath{
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

pub enum StickerType {
    Regular
}
pub enum StickerSetType {
    Regular
}

impl Bot {
    pub fn me(&self) -> &BotInfo {
        &self.info
    }

    pub async fn login(token: String) -> Result<Bot> {
        let client = reqwest::Client::new();

        let info = request_telegram::<BotInfo>(&client, &token, "getMe", &[]).await?;

        Ok(Self {
            token,
            client,
            info: Arc::new(info),
        })
    }

    pub async fn get_sticker_set(&self, name: &str) -> Result<StickerSet> {
        let sticker_set = request_telegram::<StickerSet>(
            &self.client,
            &self.token,
            "getStickerSet",
            &[("name", name)],
        )
        .await?;
        info!("stickers: {:?}", &sticker_set);

        Ok(sticker_set)
    }

    pub async fn get_url(&self, sticker: &dyn TelegramFile) -> Result<TelegramFilePath>{
        let file = request_telegram::<FileInfoResp>(
            &self.client,
            &self.token,
            "getFile",
            &[("file_id", sticker.file_id())],
        ).await?;
        Ok(TelegramFilePath(file.file_path))
    }

    pub async fn send_download_request(&self, path: &TelegramFilePath)->Result<Response>{
        let link = format!("https://api.telegram.org/file/bot{}/{}", self.token, &path.0);
        let request = self.client.get(link);
        Ok(request.send().await?)
    }
}

mod test {
    use super::*;

    fn token()->String{
        std::env::var("TSTICKER_TOKEN").unwrap()
    }


    #[tokio::test]
    async fn test_login() {
        Bot::login(token())
            .await
            .expect("Fail to login");
    }

    #[tokio::test]
    #[should_panic]
    async fn test_login_fail() {
        let fake_token = token()
            .chars()
            .map(
                |ch| match (ch.is_alphabetic(), ch.is_uppercase(), ch.is_numeric()) {
                    (true, true, _) => 'A',
                    (true, false, _) => 'a',
                    (_, _, true) => '0',
                    (_, _, _) => ch,
                },
            )
            .collect::<String>();
        Bot::login(dbg!(fake_token)).await.unwrap();
    }

    #[tokio::test]
    async fn test_list_sticker() {
        let test_list = vec![
            "myadestes_1_amashiro_natsuki_plus_nacho_neko", // plain
            "in_EDIHDC_by_NaiDrawBot",                      // animated
        ];
        let bot = Bot::login(token()).await.unwrap();
        for name in test_list {
            let sticker_set = bot.get_sticker_set(name).await.unwrap();
            println!("{:?}", sticker_set);
        }
    }
}
