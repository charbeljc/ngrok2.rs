use ngrok2::Ngrok;
fn main() {
    let ngrok = Ngrok::new();
    println!("debug: {:#?}", ngrok)
}