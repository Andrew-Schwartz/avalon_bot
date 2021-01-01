use image::io::Reader;
use tiny_skia::{Canvas, Pixmap, PixmapPaint};

#[allow(unused_imports)]
use rayon::iter::{IntoParallelIterator, ParallelIterator};

type Tokens = (Pixmap, Pixmap, Pixmap);

#[allow(non_snake_case)]
fn main() {
    std::fs::create_dir("images/avalon/board/composed").unwrap();

    let G = Pixmap::load_png("images/avalon/board/G.png").unwrap();
    let E = Pixmap::load_png("images/avalon/board/E.png").unwrap();
    let R = Pixmap::load_png("images/avalon/board/R.png").unwrap();
    let tokens = (G, E, R);

    (5_usize..=10)
        .into_par_iter()
        // .take(1)
        .map(|p| (p, Pixmap::load_png(format!("images/avalon/board/{}.png", p)).unwrap()))
        .for_each(|(p, board)| make_board(vec![], p, board, &tokens));
}

fn make_board(rounds: Vec<bool>, players: usize, mut board: Pixmap, tokens: &Tokens) {
    let fname = fname(players, &rounds, 0);
    if !rounds.is_empty() {
        let (x, y) = GE_POS[players - 5][rounds.len() - 1];
        let mut canvas = Canvas::from(board.as_mut());
        let good_win = *rounds.last().unwrap();
        let token = if good_win { tokens.0.as_ref() } else { tokens.1.as_ref() };
        canvas.draw_pixmap(x, y, token, &PixmapPaint::default());
    }
    save(&board, fname);
    let good_wins = rounds.iter().filter(|b| **b).count();
    let evil_wins = rounds.len() - good_wins;
    if good_wins < 3 && evil_wins < 3 {
        (1usize..=5)
            .into_par_iter()
            .for_each(|i| make_reject(board.clone(), &rounds, players, i, &tokens.2));
        make_board(push(rounds.clone(), true), players, board.clone(), tokens);
        make_board(push(rounds, false), players, board, tokens);
    }
}

fn make_reject(mut board: Pixmap, rounds: &[bool], players: usize, rejects: usize, token: &Pixmap) {
    let mut canvas = Canvas::from(board.as_mut());
    let (x, y) = VOTE_POS[players - 5][rejects-1];
    canvas.draw_pixmap(x, y, token.as_ref(), &PixmapPaint::default());
    save(&board, fname(players, rounds, rejects));
}

fn push<T>(mut vec: Vec<T>, t: T) -> Vec<T> {
    vec.push(t);
    vec
}

fn fname(players: usize, wins: &[bool], rejects: usize) -> String {
    let mut name = players.to_string();
    name.push_str(&wins.iter()
        .map(|&gw| if gw { 'G' } else { 'E' })
        .chain((0..rejects).map(|_| 'R'))
        .collect::<String>()
    );
    name
}

fn save(board: &Pixmap, fname: String) {
    println!("saving {:?}", fname);
    let png = format!("images/avalon/board/composed/{}.png", fname);
    board.save_png(&png).unwrap();
    let board = Reader::open(&png).unwrap().decode().unwrap();
    board.save(format!("images/avalon/board/composed/{}.jpg", fname)).unwrap();
    std::fs::remove_file(&png).unwrap();
}

pub const GE_POS: [[(i32, i32); 5]; 6] = [
    [(77, 1025), (717, 1025), (1363, 1025), (2032, 1025), (2728, 1025)],
    [(106, 946), (776, 940), (1444, 940), (2128, 934), (2780, 934)],
    [(100, 990), (786, 988), (1448, 996), (2110, 998), (2760, 998)],
    [(97, 1009), (766, 1008), (1437, 1004), (2121, 1002), (2791, 966)],
    [(120, 966), (784, 958), (1448, 962), (2094, 954), (2756, 952)],
    [(86, 1008), (734, 1002), (1394, 1000), (2042, 996), (2710, 992)],
];

pub const VOTE_POS: [[(i32, i32); 5]; 6] = [
    [(126, 1957), (596, 1970), (1070, 1970), (1555, 1985), (2046, 2003)],
    [(150, 1922), (650, 1924), (1152, 1920), (1646, 1918), (2142, 1912)],
    [(158, 1942), (660, 1946), (1148, 1938), (1636, 1940), (2124, 1952)],
    [(145, 1980), (640, 1980), (1145, 1980), (1637, 1982), (2137, 1978)],
    [(166, 1918), (650, 1916), (1154, 1906), (1638, 1902), (2114, 1906)],
    [(154, 1950), (630, 1944), (1111, 1938), (1586, 1938), (2062, 1936)],
];