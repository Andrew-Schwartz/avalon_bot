use std::fmt::{self, Debug};
use std::time::Instant;

use image::{DynamicImage, GenericImageView, Rgba};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::overlay;
use image::io::Reader;
use imageproc::geometric_transformations::{Interpolation, rotate};
use once_cell::sync::Lazy;
use rand::prelude::*;
use rand::distributions::Uniform;
use imageproc::definitions::Image;

static G: Lazy<DynamicImage> = Lazy::new(|| Reader::open("images/avalon/board/G.png").unwrap().decode().unwrap());
static E: Lazy<DynamicImage> = Lazy::new(|| Reader::open("images/avalon/board/E.png").unwrap().decode().unwrap());
static R: Lazy<DynamicImage> = Lazy::new(|| Reader::open("images/avalon/board/R.png").unwrap().decode().unwrap());
// hmm
// static G2: DynamicImage = Reader::open("").unwrap().decode().unwrap();

#[derive(Clone)]
pub struct Board(usize, DynamicImage);

impl Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Board")
            .field("players", &self.0)
            .finish()
    }
}

impl Board {
    pub fn new(players: usize) -> Self {
        // todo just decode jpg?
        let board = Reader::open(format!("images/avalon/board/{}.jpg", players))
            .unwrap().decode().unwrap();
        Self(players, board)
    }

    // todo get rid of unwraps?
    pub fn image(&self, wins: &[bool], rejects: usize) -> Vec<u8> {
        let start = Instant::now();
        let mut board = self.1.clone();
        let mut rng = thread_rng();
        const BOUND: f32 = std::f32::consts::PI / 10.0;
        let radians = Uniform::new_inclusive(-BOUND, BOUND);
        println!("cloned = {:?}", start.elapsed());
        wins.into_iter()
            .map(|&gw| if gw { &G } else { &E })
            .enumerate()
            .for_each(|(i, token)| {
                let rotated = Self::rotate_token(&mut rng, radians, &token);
                let (x, y) = GE_POS[self.0 - 5][i];
                overlay(&mut board, &rotated, x, y)
            });
        println!("G/E tokens = {:?}", start.elapsed());
        if rejects != 0 {
            let rotated = Self::rotate_token(&mut rng, radians, &R);
            let (x, y) = R_POS[self.0 - 5][rejects - 1];
            overlay(&mut board, &rotated, x, y);
        }
        println!("R tokens = {:?}", start.elapsed());
        let mut buf = Vec::new();
        JpegEncoder::new(&mut buf).encode_image(&board).unwrap();
        println!("encoded = {:?}", start.elapsed());
        buf
    }

    fn rotate_token(mut rng: &mut ThreadRng, radians: Uniform<f32>, token: &DynamicImage) -> Image<Rgba<u8>> {
        let theta = radians.sample(&mut rng);
        println!("theta = {:?}", theta);
        rotate(
            &token.to_rgba8(),
            ((token.width() / 2) as f32, (token.height() / 2) as f32),
            theta,
            Interpolation::Nearest,
            Rgba([0, 0, 0, 0]),
        )
    }
}

// todo new coords for 6-10 players
const GE_POS: [[(u32, u32); 5]; 6] = [
    [(20, 293), (203, 294), (389, 294), (592, 293), (781, 295)],
    [(68, 540), (440, 540), (821, 534), (1205, 535), (1575, 530)],
    [(61, 560), (447, 564), (825, 566), (1196, 564), (1564, 568)],
    [(60, 574), (440, 572), (818, 572), (1208, 570), (1580, 567)],
    [(72, 547), (453, 545), (818, 546), (1188, 542), (1558, 540)],
    [(53, 575), (423, 576), (794, 570), (1159, 567), (1531, 564)],
];

const R_POS: [[(u32, u32); 5]; 6] = [
    [(36, 561), (169, 563), (307, 566), (445, 570), (586, 573)],
    [(97, 1091), (376, 1091), (658, 1091), (937, 1088), (1215, 1085)],
    [(100, 1102), (380, 1106), (656, 1106), (932, 1104), (1208, 1106)],
    [(95, 1126), (372, 1120), (654, 1124), (934, 1122), (1216, 1120)],
    [(104, 1089), (383, 1086), (659, 1086), (931, 1083), (1204, 1083)],
    [(100, 1104), (365, 1102), (638, 1096), (904, 1098), (1174, 1098)],
];