mod index;

fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    println!("Hello, world!");
}
