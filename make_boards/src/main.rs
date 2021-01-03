use image::io::Reader;
use tiny_skia::{Canvas, Pixmap, PixmapPaint};

#[allow(unused_imports)]
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::io::ErrorKind;

type Tokens = (Pixmap, Pixmap, Pixmap);

#[allow(non_snake_case)]
fn main() {
    if let Err(e) = std::fs::create_dir("images/avalon/board/composed") {
        if e.kind() != ErrorKind::AlreadyExists {
            panic!("{}", e)
        }
    }

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
    [(45, 583), (408, 583), (776, 588), (1155, 585), (1546, 582)],
    [(68, 540), (440, 540), (821, 534), (1205, 535), (1575, 530)],
    [(61, 560), (447, 564), (825, 566), (1196, 564), (1564, 568)],
    [(60, 574), (440, 572), (818, 572), (1208, 570), (1580, 567)],
    [(72, 547), (453, 545), (818, 546), (1188, 542), (1558, 540)],
    [(53, 575), (423, 576), (794, 570), (1159, 567), (1531, 564)],
];

pub const VOTE_POS: [[(i32, i32); 5]; 6] = [
    [(83, 1114), (345, 1119), (612, 1122), (887, 1129), (1164, 1135)],
    [(97, 1091), (376, 1091), (658, 1091), (937, 1088), (1215, 1085)],
    [(100, 1102), (380, 1106), (656, 1106), (932, 1104), (1208, 1106)],
    [(95, 1126), (372, 1120), (654, 1124), (934, 1122), (1216, 1120)],
    [(104, 1089), (383, 1086), (659, 1086), (931, 1083), (1204, 1083)],
    [(100, 1104), (365, 1102), (638, 1096), (904, 1098), (1174, 1098)],
];