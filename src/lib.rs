use serde_json::Value;
use url::Url;
use log::{debug, error, info, trace, warn};
use std::{fs, io::prelude::*, str, thread::JoinHandle};
use std::io::BufReader;
use std::{fs::File, io::Read};
use std::io::BufWriter;
use std::env;
use std::path::{Path, PathBuf};
use std::cfg;
use std::{thread, time};
use std::process;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use ureq;

use unzip;
#[macro_use]
extern crate error_chain;
error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    errors {
        Server(status: u16, text: String) {
            description("HTTP Error")
            display("HTTP Error: {}: {}", status, text)
        }
    }
    foreign_links {
        UReq(ureq::Error);
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
    base_url: Url,
}
pub fn find_file_in_path<P>(exe_name: P) -> Option<PathBuf>
    where P: AsRef<Path>,
{
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).filter_map(|dir| {
            let full_path = dir.join(&exe_name);
            if full_path.is_file() {
                Some(full_path)
            } else {
                None
            }
        }).next()
    })
}

impl Ngrok {
    pub fn new() -> Self {
       Ngrok {
           base_url: Url::parse(BASE_URL_STR).unwrap(),
       } 
    }

    pub fn exe_name(&self) -> String {
        format!("ngrok{}", env::consts::EXE_SUFFIX)
    }
    pub fn start(&self) -> Result<(Tunnels, Option<process::Child>)> {
        let mut child: Option<process::Child> = None;
        for probe in 0..7 {
            debug!("STARTING ATTEMPT: {}", probe);

            let tunnels = match self.get::<Tunnels>("api/tunnels") {
                Ok(resp) =>  Ok(resp),
                Err(err) => {
                    match child {
                        None => {
                            debug!("NO THREAD");
                            if let Ok(handle) = self.start_server() {
                                debug!("SERVER STARTED!!");
                                child = Some(handle);
                            } else {
                                debug!("FAIL!!")
                            }
                        }
                        Some(_) => {
                            debug!("ALREADY STARTED");
                        }
                    }
                    Err(err)
                }
            };
            if let Ok(tunnels) = tunnels {
                return Ok((tunnels, child));
            } else {
                let ten_millis = time::Duration::from_millis(10);
                let now = time::Instant::now();
                thread::sleep(ten_millis);
                debug!("now elapsed: {:?}", now.elapsed());
            }
        }
        Err(Error::from_kind(ErrorKind::Msg("could not start ngrok".to_owned())))
    }

    pub fn start_server(&self) -> Result<process::Child> {

        let path = match find_file_in_path(self.exe_name()) {
            None => {
                debug!("no ngrok executable found");
                self.download()
            }
            Some(path) => Some(path)
        };
        debug!("START-SERVER: {:#?}", path);
        let res = match path {
            Some(path) => {
                info!("launching ngrok: {}", path.to_string_lossy());
                let mut proc = process::Command::new(path)
                    .args(&["start", "--none"])
                    .spawn()
                    .expect("ngrok failed to start");
                info!("ngrok started: {:#?}", proc);

                //     let ecode = proc.wait()
                //         .expect("failed to wait on child");
                //     debug!("ret code: {}", ecode);
                debug!("CHILD: {:#?}", proc);
                Ok(proc)
            }
            None => {
                debug!("could not find ngrok executable");
                Err(Error::from_kind(ErrorKind::Msg("fooo".to_owned()))) 
            }
        };
        res
    }

    pub fn get<T>(&self, path: &str) -> Result<T> where T: std::fmt::Debug + for<'de> Deserialize<'de> {
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
                            Err(e) => Err(Error::from_kind(ErrorKind::Msg(e.to_string())))
                        }
                    }
                    Err(err) => {
                        debug!("get err: {:?}", err);
                        Err(Error::from("get err"))
                    }
                }
            }
            Err(err) => Err(err)
        }
    } 
    pub fn post<T>(&self, path: &str, data: Value) -> Result<T> where T: std::fmt::Debug + for<'de> Deserialize<'de> {
        match self.base_url.join(path).chain_err(|| "oups1") {
            Ok(url) => {
                match ureq::post(url.as_str()).send_json(data) {
                    Ok(resp) => {
                        match resp.status() {
                            200 | 201 => {
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
                                    Err(e) => Err(Error::from_kind(ErrorKind::Msg(e.to_string())))
                                }
                            }
                            error => {
                                let status = resp.status();
                                let text = resp.status_text().to_owned();
                                let body: Value = resp.into_json().unwrap();
                                error!("code: {}: {}\nbody: {:#?}", status, text, body);
                                Err(Error::from_kind(ErrorKind::Server(status, text)))
                            }
                        }
                    }
                    Err(err) => {
                        error!("post err: {:?}", err);
                        Err(Error::from_kind(ErrorKind::UReq(err)))
                    }
                }
            }
            Err(err) => Err(err)
        }
    } 

   pub fn delete(&self, path: &str) -> Result<()> {
        match self.base_url.join(path).chain_err(|| "oups1") {
            Ok(url) => {
                match ureq::delete(url.as_str()).call() {
                    Ok(resp) => {
                        match resp.status() {
                            204 => Ok(()),
                            status => {
                                error!("HTTP CODE! {}", status);
                                Err(Error::from_kind(ErrorKind::Server(status, "uu".to_owned())))
                            }
                        }
                    }
                    Err(err) => {
                        error!("post err: {:?}", err);
                        Err(Error::from_kind(ErrorKind::UReq(err)))
                    }
                }
            }
            Err(err) => Err(err)
        }
    }  // I'd probably grab the environment variable and iterate through it, returning the first matching path:

    pub fn download(&self) -> Option<PathBuf> {
        debug!("DONLOAD ARCH: {}, OS: {}", env::consts::ARCH, env::consts::OS);
        let url = match (env::consts::ARCH, env::consts::OS) {
            ("x86_64", "windows") => Ok(NGROK_WIN64),
            ("x86", "windows") => Ok(NGROK_WIN32),
            (_, "macos") => Ok(NGROK_MACOS),
            ("x86_64", "linux") => Ok(NGROK_LINUX),
            ("x86", "linux") => Ok(NGROK_LINUX32),
            ("aarch64", "linux") => Ok(NGROK_ARMLINUX),
            ("arm", "linux") => Ok(NGROK_ARMLINUX32),
            ("x86_64", "freebsd") => Ok(NGROK_FREEBSD),
            ("x86", "freebsd") => Ok(NGROK_FREEBSD32),
            (_, _) => Err(Error::from_kind(ErrorKind::Msg("huuu".to_owned())))
        };
        debug!("ngrok dl url: %{:?}", url);

        match url {
            Err(err) => {
                debug!("ngrok is not supported on {} {}: {}", env::consts::ARCH, env::consts::OS, err);
                None
            }
            Ok(url) => {
                match ureq::get(url).call() {
                    Ok(resp) => {
                        let raw_reader = resp.into_reader();
                        let mut reader = BufReader::new(raw_reader);
                        let mut buf=[0u8;1024];
                        let zip = File::create("ngrok-local.zip").unwrap();
                        let mut writer = BufWriter::new(zip);
                        while let Ok(n) = reader.read(&mut buf) {
                            if n == 0 {
                                break
                            }
                            writer.write(&buf[..n]).unwrap();
                        }
                        writer.flush().unwrap();
                        let zip = File::open("ngrok-local.zip").unwrap();
                        let reader = BufReader::new(zip);
                        let path = Path::new(".");
                        let unz = unzip::Unzipper::new(reader, &path);
                        let _stats = unz.unzip().unwrap();
                        let exe_name = format!("ngrok{}", env::consts::EXE_SUFFIX);
                        let exe_path = PathBuf::from(exe_name);
                        let exe_path = if exe_path.is_absolute() {
                            exe_path
                        } else {
                            fs::canonicalize(exe_path).unwrap()
                        };
                        let exe = File::open(&exe_path).unwrap();
                        #[cfg(target_os = "linux")]
                        {
                            info!("OK Linux");
                            info!("making {:?} executable", exe);
                            let metadata = exe.metadata().unwrap();
                            let mut permissions = metadata.permissions();
                            debug!("permissions: {:#?}", permissions.mode());
                            permissions.set_mode(0o700);
                            assert_eq!(permissions.mode(), 0o700);
                            debug!("permissions: {:#?}", permissions.mode());
                            match exe.set_permissions(permissions) {
                                Ok(res) => {
                                    info!("exec permission set on {:?}: {:?}", exe, res);
                                }
                                Err(e) => {
                                    error!("could not make {:?} executable: {:?}", exe, e);
                                }
                            };
                        }
                        Some(exe_path)
                    }
                    Err(_) => None
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::{debug, info, warn, error};
    use serde::__private::ser;
    use crate::{Ngrok, Tunnel, Tunnels, find_file_in_path}; 
    use serde_json::{Value, json};
    use env_logger;
    use std::{process, sync::Once};
    use std::thread;
    use std::time;
//    lazy_static! {
//        statuc FOO = env_logger::init();
//    }
    static START: Once = Once::new();
    fn setup() {
        START.call_once(|| {
            //env_logger::init();
            env_logger::init();
            error!("E");
            warn!("W");
            info!("I");
            debug!("D");
        });
            error!("E");
            warn!("W");
            info!("I");
            debug!("D");
    }
    #[test]
    fn get_tunnels() {
        setup();
        let ngrok = Ngrok::new();
        let tunnels = ngrok.start();
        match tunnels {
            Ok((tunnels, join)) => {
                assert_eq!(tunnels.tunnels.len(), 0);
                let erp_tunnel = json!({
                        "name": "erp",
                        "addr": 8069,
                        "proto": "http",
                        "bind_tls": "both",
                        "inspect": true
                });
                let ota_tunnel= json!({
                        "name": "ota",
                        "addr": 1999,
                        "proto": "http",
                        "bind_tls": true,
                        "inspect": true
                });
 
                info!("creating tunnels...");
                for probe in 0..7 {
                    debug!("GT PROBE: {}", probe);
                    match ngrok.post::<Value>("api/tunnels", erp_tunnel.to_owned()) {
                        Ok(value) => {
                            match serde_json::from_value::<Tunnel>(value) {
                                Ok(tunnel) => {
                                    info!("new tunnel: {}", tunnel.name);
                                    break
                                }
                                Err(err) => {
                                    error!("could not deserialize tunnel: {:?}", err);
                                }
                            }
                        }
                        Err(err) => {
                            error!("malformed response: {:?}", err);
                            let ten_millis = time::Duration::from_secs(1);
                            let now = time::Instant::now();
                            thread::sleep(ten_millis);
                            debug!("now elapsed: {:?}", now.elapsed());
                        }
                    };
                }
                match ngrok.post::<Value>("api/tunnels", ota_tunnel.to_owned()) {
                    Ok(value) => {
                        match serde_json::from_value::<Tunnel>(value) {
                            Ok(tunnel) => {
                                info!("new tunnel: {:#?}", tunnel);
                            }
                            Err(err) => {
                                error!("could not deserialize tunnel: {:?}", err);
                            }
                        }
                    }
                    Err(err) => {
                        error!("malformed response: {:?}", err);
                        let ten_millis = time::Duration::from_secs(1);
                        let now = time::Instant::now();
                        thread::sleep(ten_millis);
                        debug!("now elapsed: {:?}", now.elapsed());
                    }
                }
                info!("getting tunnels");

                match ngrok.get::<Tunnels>("api/tunnels") {
                    Ok(tunnels) => {
                        info!("Tunnels # {}", tunnels.tunnels.len());
                        assert_eq!(tunnels.tunnels.len(), 3);
                        for tun in tunnels.tunnels {
                            info!("name: {}", tun.name);
                            info!("deletting  tunnels");
                            match ngrok.delete(&format!("api/tunnels/{}", tun.name)) {
                                Ok(()) => {
                                    info!("delete ok");


                                }
                                Err(err) => {
                                    error!("could not delete tunnels: {:?}", err);
                                }
                            }
                        }
                        match ngrok.get::<Tunnels>("api/tunnels") {
                            Ok(tunnels) => {
                                assert_eq!(tunnels.tunnels.len(), 0);
                            }
                            Err(err) => {
                                error!("could not get tunnels: {:?}", err);
                            }
                        };

                    }
                    Err(err) => {
                        error!("could not get tunnels {:?}", err);
                    }
                };


                if let Some(mut child) = join {
                    info!("Waiting process to finish");
                    child.kill().unwrap();
                    info!("Done.")
                }
            }

            Err(err) => {
                error!("could not start ngrok: {:#?}", err);
                assert_eq!(true, false);
            }
        }
    }
}

