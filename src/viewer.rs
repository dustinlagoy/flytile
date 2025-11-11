use crate::tile;
use maud::html;
use maud::Markup;

pub fn viewer(zoom: u8, x: u32, y: u32) -> Markup {
    html! {
        html {
            head {link rel="stylesheet" href="https://lagoy.org/tiles/css/style.css";}
            body {
                iframe hidden name="htmz" onload="setTimeout(()=>document.querySelector(contentWindow.location.hash||null)?.replaceWith(...contentDocument.body.childNodes))" {}
                h1 {"Tile Viewer"}
                (image_grid(zoom, x, y, 9, 5))
            }
        }
    }
}

pub fn image_grid(zoom: u8, x: u32, y: u32, nx: u32, ny: u32) -> Markup {
    let dy: i32 = (ny / 2) as i32;
    let dx: i32 = (nx / 2) as i32;
    html! {
        div #viewer .viewer {
            div .viewhead {
                form target="htmz" {
                    button formaction=(format!("https://lagoy.org/tiles/grid/{}/{}/{}.png#viewer", zoom+1, x*2, y*2)) target="htmz" {"zoom in"}
                    " "
                    button formaction=(format!("https://lagoy.org/tiles/grid/{}/{}/{}.png#viewer", zoom-1, x/2, y/2)) target="htmz" {"zoom out"}
                }
            }
            div .viewgrid {
                @for iy in -dy..dy+1 {
                    @let yy = y as i32 + iy;
                    @for ix in -dx..dx+1 {
                        @let xx = x as i32 + ix;
                        div class=(format!("vi{:02}{:02}",iy,ix)) {
                          a href=(format!("https://lagoy.org/tiles/grid/{}/{}/{}.png#viewer", zoom, xx, yy))
                          target="htmz" {
                            embed type="image/png" src=(format!(
                                "https://lagoy.org/tiles/imagery/latest/{}/{}/{}.png",
                                zoom, xx, yy));
                          }
                        }
                    }
                }
            }
        }
    }
}
// <!DOCTYPE html>
// <html>
// <!-- <script src="https://unpkg.com/htmx.org@2.0.4"></script> -->
// <!-- <meta name="htmx-config" content='{"selfRequestsOnly":false}'> -->
// <head>
// <title>Page Title</title>
// </head>
// <body>
// <iframe hidden name=htmz onload="setTimeout(()=>document.querySelector(contentWindow.location.hash||null)?.replaceWith(...contentDocument.body.childNodes))"></iframe>

// <!-- <div hx-request='{"noHeaders": true}'> -->
// <h1>This is a Heading</h1>
// <p>This is a paragraph.</p>
// <!-- <button hx-request='{"noHeaders": true}', hx-get="https://lagoy.org/tiles/imagery/latest/12/669/1398.png">Load Todos</button> -->

// <a href="https://lagoy.org/tiles/imagery/latest/12/669/1398.png#image" target="htmz">8</a>

// <div id="image">
// <embed type="image/png" src="https://lagoy.org/tiles/imagery/latest/12/669/1399.png">
// </div>

// </body>
// </html>
