use std::{env, fs};
use std::io::{Cursor, stderr, stdout};
use std::process::{Command, Stdio};
use json::JsonValue;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};

static URL: &'static str = "https://api.github.com/repos/BigBadE/Raven-Language/actions/artifacts";
static TOKEN: &'static str = "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";

fn main() {
    println!("Checking version...");
    // Add headers to the HTTP request
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", HeaderValue::from_str(
        format!("Bearer {}", TOKEN).as_str()).unwrap());
    let client = Client::builder().user_agent("Magpie Updater")
        .default_headers(headers).build().unwrap();
    // Get the artifacts
    let body = client.get(URL).send().unwrap().text().unwrap();
    let parsed = json::parse(body.as_str()).unwrap();

    // Print the error, if any
    if !parsed["message"].is_null() {
        panic!("{}", parsed["message"].as_str().unwrap());
    }

    // Find the latest artifacts
    let mut highest: Option<&JsonValue> = None;
    for artifact in parsed["artifacts"].members() {
        if artifact["name"].as_str().unwrap() != format!("Raven-{}", env::consts::OS) {
            continue
        }
        if let Some(found) = &highest {
            if artifact["id"].as_u64() > found["id"].as_u64() {
                highest = Some(artifact);
            }
        } else {
            highest = Some(artifact);
        }
    }

    if highest.is_none() {
        panic!("No Magpie version found! Make sure your OS is supported ({} must be linux, macos, or windows).", env::consts::OS)
    }

    let highest = highest.unwrap();

    let running = env::temp_dir().join(format!("magpie-{}.{}", highest["id"], env::consts::EXE_EXTENSION));

    // If latest is not already downloaded, download it.
    if !running.exists() {
        println!("Downloading new Magpie version...");
        let download = highest["archive_download_url"].as_str().unwrap();
        zip_extract::extract(Cursor::new(client.get(download).send().unwrap().bytes().unwrap()),
                             &env::temp_dir(), false).unwrap();
        let target = env::temp_dir().join(format!("magpie.{}", env::consts::EXE_EXTENSION));
        fs::rename(target, running.clone()).unwrap();
    }

    Command::new(running).stdout(stdout()).stdin(Stdio::inherit())
        .stderr(stderr()).output().unwrap();
}