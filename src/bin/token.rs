use flytile::token;

fn main() {
    env_logger::init();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let generator = token::Generator::new(
        "https://identity.dataspace.copernicus.eu/auth/realms/CDSE/protocol/openid-connect/token",
    );

    println!("{}", runtime.block_on(generator.get()).unwrap());
    // this should return the cached token
    println!("{}", runtime.block_on(generator.get()).unwrap());
}
