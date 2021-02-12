use std::fmt::{self, Debug};

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
        let board = Reader::open(format!("images/avalon/board/{}.jpg", players))
            .unwrap().decode().unwrap();
        Self(players, board)
    }

    // todo get rid of unwraps?
    pub fn image(&self, wins: &[bool], rejects: usize) -> Vec<u8> {
        const BOUND: f32 = std::f32::consts::PI / 10.0;

        let mut board = self.1.clone();
        let mut rng = thread_rng();
        let radians = Uniform::new_inclusive(-BOUND, BOUND);
        wins.iter()
            .map(|&gw| if gw { &G } else { &E })
            .enumerate()
            .for_each(|(i, token)| {
                let rotated = Self::rotate_token(&mut rng, radians, token);
                let (x, y) = GE_POS[self.0 - 5][i];
                overlay(&mut board, &rotated, x, y)
            });
        if rejects != 0 {
            let rotated = Self::rotate_token(&mut rng, radians, &R);
            let (x, y) = R_POS[self.0 - 5][rejects - 1];
            overlay(&mut board, &rotated, x, y);
        }
        let mut buf = Vec::new();
        JpegEncoder::new(&mut buf).encode_image(&board).unwrap();
        buf
    }

    fn rotate_token(mut rng: &mut ThreadRng, radians: Uniform<f32>, token: &DynamicImage) -> Image<Rgba<u8>> {
        let theta = radians.sample(&mut rng);
        #[allow(clippy::cast_precision_loss)]
        rotate(
            &token.to_rgba8(),
            ((token.width() / 2) as f32, (token.height() / 2) as f32),
            theta,
            Interpolation::Nearest,
            Rgba([0, 0, 0, 0]),
        )
    }
}

const GE_POS: [[(u32, u32); 5]; 6]  = [
    [(20, 293), (203, 294), (389, 294), (582, 293), (781, 295)],
    [(30, 271), (221, 270), (413, 268), (607, 266), (794, 266)],
    [(29, 281), (221, 283), (413, 283), (602, 284), (790, 284)],
    [(26, 288), (317, 287), (411, 286), (606, 284), (798, 284)],
    [(33, 274), (224, 273), (411, 271), (599, 272), (786, 271)],
    [(23, 287), (210, 287), (398, 286), (585, 285), (773, 283)],
];

const R_POS: [[(u32, u32); 5]; 6] = [
    [(36, 561), (169, 563), (307, 566), (445, 570), (586, 573)],
    [(44, 550), (187, 549), (330, 548), (472, 546), (613, 546)],
    [(47, 557), (189, 556), (328, 556), (468, 555), (608, 558)],
    [(45, 565), (184, 566), (327, 567), (470, 566), (613, 566)],
    [(48, 549), (189, 545), (330, 546), (470, 544), (607, 545)],
    [(45, 557), (182, 557), (320, 554), (455, 554), (592, 553)],
];