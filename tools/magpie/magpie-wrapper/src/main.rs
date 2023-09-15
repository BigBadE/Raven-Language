use std::{env, fs};
use json::{JsonValue, object};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};

static URL: &'static str = "https://api.github.com/repos/BigBadE/Raven-Language/actions/artifacts";
static TOKEN: &'static str = "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";

fn main() {
    let runner = env::current_exe().unwrap().parent().unwrap().join("magpie-runner");

    if runner.exists() {

    }

    check_version();
}

fn check_version() {
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", HeaderValue::from_str(
        format!("Bearer {}", TOKEN).as_str()).unwrap());
    let client = Client::builder().user_agent("Magpie Updater").default_headers(headers).build().unwrap();
    let body = client.get(URL).send().unwrap().text().unwrap();
    let parsed = json::parse(body.as_str()).unwrap();
    let mut highest: Option<&JsonValue> = None;
    for artifact in parsed["artifacts"].members() {
        if let Some(found) = &highest {
            if artifact["id"].as_u64() > found["id"].as_u64() {
                highest = Some(artifact);
            }
        } else {
            highest = Some(artifact);
        }
    }

    println!("URL: {}", highest.unwrap()["archive_download_url"]);
    let download = highest.unwrap()["archive_download_url"].as_str().unwrap();
    let temp = env::temp_dir().join("magpie-download.zip");
    fs::write(temp.clone(), client.get(download).send().unwrap().bytes().unwrap()).unwrap();
    println!("Written to {}", temp.to_str().unwrap());
}