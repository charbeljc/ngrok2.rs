use error_chain::ChainedError;
use serde_json::Value;
use serde;
use url::Url;
use ureq;
use log::debug;

#[macro_use]
extern crate error_chain;
error_chain! {
    errors {
    }
}

static BASE_URL_STR: &str  = "http://127.0.0.1:4040";

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
                        Err(Error::from("hammp"))
                    }
                }
            }
            Err(err) => Err(err)
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

