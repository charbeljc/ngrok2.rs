use serde_json::Value;
use url::Url;
use ureq;
use log::debug;
use std::{io::prelude::*, str};
use std::io::BufReader;
use std::{fs::File, io::Read};
use std::io::BufWriter;
use std::path::Path;
use std::env;

use unzip;
#[macro_use]
extern crate error_chain;
error_chain! {
    errors {
    }
}
use serde::{Deserialize};

static BASE_URL_STR: &str  = "http://127.0.0.1:4040";
static NGROK_WIN64: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-windows-amd64.zip";
static NGROK_WIN32: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-windows-386.zip";
static NGROK_MACOS: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-darwin-amd64.zip";
static NGROK_LINUX: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-linux-amd64.zip";
static NGROK_LINUX32: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-linux-386.zip";
static NGROK_ARMLINUX: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-linux-arm64.zip";
static NGROK_ARMLINUX32: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-linux-arm.zip";
static NGROK_FREEBSD: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-freebsd-amd64.zip";
static NGROK_FREEBSD32: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-freebsd-386.zip";

#[derive(Debug, Deserialize)]
pub struct BaseMetric {
    count: u64,
    rate1: f64,
    rate5: f64,
    rate15: f64,
    p50: f64,
    p90: f64,
    p95: f64,
    p99: f64
}
#[derive(Debug, Deserialize)]
pub struct GaugeMetric {
    count: u64,
    rate1: f64,
    rate5: f64,
    rate15: f64,
    p50: f64,
    p90: f64,
    p95: f64,
    p99: f64,
    gauge: f64,
}

#[derive(Debug, Deserialize)]
pub struct Metrics {
    conns: GaugeMetric,
    http: BaseMetric,
}

#[derive(Debug, Deserialize)]
pub struct TunnelConfig {
    addr: String,
    inspect: bool,
}
#[derive(Debug, Deserialize)]
pub struct Tunnel {
    name: String,
    uri: String,
    public_url: String,
    proto: String,
    config: TunnelConfig,
    metrics: Metrics 
}

#[derive(Debug, Deserialize)]
pub struct Tunnels {
    tunnels: Vec<Tunnel>
}

#[derive(Debug)]
pub struct Ngrok {
    base_url: Url
}

impl Ngrok {
    pub fn new() -> Self {
       Ngrok {
           base_url: Url::parse(BASE_URL_STR).unwrap(),
       } 
    }
    pub fn call<T>(&self, path: &str) -> Result<T> where T: std::fmt::Debug + for<'de> Deserialize<'de> {
        match self.base_url.join(path).chain_err(|| "oups1") {
            Ok(url) => {
                match ureq::get(url.as_str()).call() {
                    Ok(resp) => {
                        assert!(resp.has("Content-Length"));
                        let len = resp.header("Content-Length")
                            .and_then(|s| s.parse::<usize>().ok()).unwrap();
                        
                        let mut bytes: Vec<u8> = Vec::with_capacity(len);
                        resp.into_reader()
                            .read_to_end(&mut bytes).chain_err(|| "unable to read data");
                        assert_eq!(bytes.len(), len);
                        let string = str::from_utf8(&bytes);
                        match string {
                            Ok(string) => {
                                match serde_json::from_str::<Value>(string) {
                                    Ok(json) => {
                                        match serde_json::from_value::<T>(json.to_owned()) {
                                            Ok(res) => {
                                                Ok(res)
                                            }
                                            Err(err) => {
                                                debug!("Error: {}", err);
                                                debug!("RAW: {:#?}", json);
                                                Err(Error::from_kind(ErrorKind::Msg("huu ...".to_string())))
                                            }
                                        }
                                    }
                                    Err(e) => Err(Error::from_kind(ErrorKind::Msg("huu huu".to_string())))
                                }
                            }
                            Err(e) => Err(Error::from_kind(ErrorKind::Msg("huu huuu".to_string())))
                        }
                    }
                    Err(err) => {
                        debug!("CALL ERR: {:#?}", err);
                        self.download();
                        Err(Error::from("hammp"))
                    }
                }
            }
            Err(err) => Err(err)
        }
    } 
    pub fn download(&self) {
        debug!("ARCH: {}, OS: {}", env::consts::ARCH, env::consts::OS);
        let url = match (env::consts::ARCH, env::consts::OS) {
            ("x86_64", "windows") => Ok(NGROK_WIN64),
            ("x86", "windows") => Ok(NGROK_WIN32),
            (_, "macos") => Ok(NGROK_MACOS),
            ("x86_64", "linux") => Ok(NGROK_LINUX),
            ("x86", "linux") => Ok(NGROK_LINUX32),
            ("x86_64", "freebsd") => Ok(NGROK_FREEBSD),
            ("x86", "freebsd") => Ok(NGROK_FREEBSD32),
            (_, _) => Err(Error::from_kind(ErrorKind::Msg("huuu".to_owned())))
        };
        debug!("ngrok dl url: %{:?}", url);

        match url {
            Err(err) => {
                debug!("ngrok is not supported on {} {}: {}", env::consts::ARCH, env::consts::OS, err);
            }
            Ok(url) => {
                match ureq::get(url).call() {
                    Ok(resp) => {
                        let raw_reader = resp.into_reader();
                        let mut reader = BufReader::new(raw_reader);
                        let mut buf=[0u8;1024];
                        let zip = File::create("ngrok-win64.zip").unwrap();
                        let mut writer = BufWriter::new(zip);
                        while let Ok(n) = reader.read(&mut buf) {
                            if n == 0 {
                                break
                            }
                            writer.write(&buf[..n]).unwrap();
                        }
                        writer.flush().unwrap();
                        let zip = File::open("ngrok-win64.zip").unwrap();
                        let reader = BufReader::new(zip);
                        let path = Path::new(".");
                        let unz = unzip::Unzipper::new(reader, &path);
                        let _stats = unz.unzip().unwrap();
                    }
                    Err(_) => {}
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use ureq;
    use log::{debug, info};
    use crate::{Ngrok, Tunnels}; 
    use env_logger;
    use std::sync::Once;
//    lazy_static! {
//        statuc FOO = env_logger::init();
//    }
    static START: Once = Once::new();
    fn setup() {
        START.call_once(|| {
            env_logger::init();
        })
    }
    #[test]
    fn get_tunnels() {
        setup();
        let ngrok = Ngrok::new();

        match ngrok.call::<Tunnels>("api/tunnels") {
            Ok(resp) => {
                info!("RESP: {:#?}", resp);
                debug!("RESP: {:#?}", resp);
            }
            Err(err) => {
                info!("ERR: {:#?}", err);
                debug!("ERR: {:#?}", err);
            }
        }
    }
}

