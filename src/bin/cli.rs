use clap::{Args, Parser, Subcommand};
use flytile::tile;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    ToTile(ToTileArgs),
    ToGeo(ToGeoArgs),
}

#[derive(Args, Debug)]
struct ToTileArgs {
    zoom: u8,
    longitude: f64,
    latitude: f64,
}

#[derive(Args, Debug)]
struct ToGeoArgs {
    zoom: u8,
    x: u32,
    y: u32,
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::ToTile(args) => {
            let point = tile::GeoPoint {
                longitude: args.longitude,
                latitude: args.latitude,
            };
            println!("input point:          {:?}", point);
            let square = tile::geodetic_to_square(&point);
            println!("point on unit square: {:?}", square);
            let tile_point = tile::square_to_tile(args.zoom, &square);
            println!("tile at zoom {}:      {:?}", args.zoom, tile_point);
        }
        Commands::ToGeo(args) => {
            let square = tile::tile_to_square(args.zoom, args.x as f64 + 0.5, args.y as f64 + 0.5);
            println!("tile center on unit square: {:?}", square);
            let point = tile::square_to_geodetic(&square);
            println!("tile geodetic center:       {:?}", point);
            let bounds = tile::tile_bounds(args.zoom, args.x, args.y);
            println!("tile north west corner:     {:?}", bounds.north_west);
            println!("tile north east corner:     {:?}", bounds.north_east);
            println!("tile south west corner:     {:?}", bounds.south_west);
            println!("tile south east corner:     {:?}", bounds.south_east);
            let meters_nw = tile::square_to_meters(&tile::tile_to_square(
                args.zoom,
                args.x as f64,
                args.y as f64,
            ));
            let meters_se = tile::square_to_meters(&tile::tile_to_square(
                args.zoom,
                args.x as f64 + 1.0,
                args.y as f64 + 1.0,
            ));
            println!("tile north west corner:     {:?}", meters_nw);
            println!("tile south east corner:     {:?}", meters_se);
        }
    }
}
