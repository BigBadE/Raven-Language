use json::JsonValue;
use reqwest::blocking::Client;
use std::process::{Command, Stdio};
use std::{env, fs};

static URL: &str = "https://api.github.com/repos/BigBadE/Raven-Language/releases/123226271/assets";

fn main() {
    println!("Checking version...");
    let client = Client::builder()
        .user_agent("Magpie Updater")
        .build()
        .unwrap();
    // Get the artifacts
    let body = client.get(URL).send().unwrap().text().unwrap();
    let parsed = json::parse(body.as_str()).unwrap();

    // Print the error, if any
    if !parsed["message"].is_null() {
        panic!("{}", parsed["message"].as_str().unwrap());
    }

    // Find the latest artifacts
    let mut highest: Option<&JsonValue> = None;
    for artifact in parsed.members() {
        if artifact["name"].as_str().unwrap()
            != format!("Magpie-{}{}", env::consts::OS, env::consts::EXE_SUFFIX)
        {
            continue;
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
        panic!(
            "No Magpie version found! Make sure your OS is supported ({} must be linux, macos, or windows).",
            env::consts::OS
        )
    }

    let highest = highest.unwrap();

    let running = env::temp_dir().join(format!(
        "magpie-{}.{}",
        highest["id"],
        env::consts::EXE_EXTENSION
    ));

    // If latest is not already downloaded, download it.
    if !running.exists() {
        println!("Deleting old Magpie versions...");
        for file in fs::read_dir(env::temp_dir()).unwrap() {
            let file = file.unwrap();
            if file.file_name().to_str().unwrap().starts_with("magpie-") {
                fs::remove_file(file.path()).unwrap();
            }
        }
        println!("Downloading new Magpie version...");
        let download = highest["browser_download_url"].as_str().unwrap();
        fs::write(
            running.clone(),
            client.get(download).send().unwrap().bytes().unwrap(),
        )
        .unwrap();
    }

    Command::new(running)
        .args(env::args().into_iter().skip(1).collect::<Vec<_>>())
        .stdout(Stdio::inherit())
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .unwrap();
}
