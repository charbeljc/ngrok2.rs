use error_chain::ChainedError;
use serde_json::Value;
use serde;
use url::Url;
use ureq;
use log::debug;
use std::{fs::File, io::Read};
use std::io::Write;
use std::io::BufWriter;
use unzip;
#[macro_use]
extern crate error_chain;
error_chain! {
    errors {
    }
}

static BASE_URL_STR: &str  = "http://127.0.0.1:4040";
static NGROK_WIN64: &str = "https://bin.equinox.io/c/4VmDzA7iaHb/ngrok-stable-windows-amd64.zip";

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
    pub fn call(&self, path: &str) -> Result<Value> {
        match self.base_url.join(path).chain_err(|| "oups1") {
            Ok(url) => {
                match ureq::get(url.as_str()).call() {
                    Ok(resp) => {
                        let reader = resp.into_reader();
                        let res = serde_json::from_reader(reader).chain_err(|| "oups3");
                        debug!("CALL RES: {:#?}", res);
                        res
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
        match ureq::get(NGROK_WIN64).call() {
            Ok(resp) => {
                let mut reader = resp.into_reader();
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
            }
            Err(_) => {}
        }
    }
}


#[cfg(test)]
mod tests {
    use ureq;
    use log::{debug, info};
    use crate::Ngrok; 
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
    fn it_works() {
        setup();
        let resp  = ureq::get("http://127.0.0.1:4040/api/tunnels")
            .call().unwrap();
        let status = resp.status();
        assert_eq!(status, 200);

        info!("RESP: {:#?}", resp);
        debug!("RESP: {:#?}", resp);
    }
    #[test]
    fn it_works_too() {
        setup();
        let ngrok = Ngrok::new();

        match ngrok.call("api/tunnels") {
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

