use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Puzzle {
    pub id: u32,
    pub power: u32,
    pub address: String,
    pub pubkey: Option<String>,
    pub description: String,
}

pub fn get_active_puzzles() -> Vec<Puzzle> {
    vec![
        Puzzle {
            id: 66,
            power: 66,
            address: "13zb1hQbWVsc2S7ZTZnP2G4undNNpdh5so".to_string(),
            pubkey: None,
            description: "Puzzle #66 - Range 2^65 to 2^66 (pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 67,
            power: 67,
            address: "1BY8GQbnueYofwSuFAT3USAhGjPrkxDdW9".to_string(),
            pubkey: None,
            description: "Puzzle #67 - Range 2^66 to 2^67 (pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 68,
            power: 68,
            address: "1MVDYgVaSN6iKKEsbzRUAYFrYJadLYZvvZ".to_string(),
            pubkey: None,
            description: "Puzzle #68 - Range 2^67 to 2^68 (pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 69,
            power: 69,
            address: "19vkiEajfhuZBBbsHZ8Jd2KYz5q3sZ5G9g".to_string(),
            pubkey: None,
            description: "Puzzle #69 - Range 2^68 to 2^69 (pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 70,
            power: 70,
            address: "1PWo3JeB9jrGwfHDNpdGK54CRas7fsVzXU".to_string(),
            pubkey: None,
            description: "Puzzle #70 - Range 2^69 to 2^70 (pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 71,
            power: 71,
            address: "1JhCQPhF6T4shR8ojJ8Yr5vgc4N6MxxHQz".to_string(),
            pubkey: None,
            description: "Puzzle #71 - Range 2^70 to 2^71 (pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 72,
            power: 72,
            address: "1ACNaY7JquJ7Z3WgkRUtDCEmF23DGZ1Biw".to_string(),
            pubkey: None,
            description: "Puzzle #72 - Range 2^71 to 2^72 (pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 73,
            power: 73,
            address: "1Fz63cJc2TwZAi1SR1tGV1tmCJeTqGCWe9".to_string(),
            pubkey: None,
            description: "Puzzle #73 - Range 2^72 to 2^73 (pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 74,
            power: 74,
            address: "1FWGcVDK3JGzCC3WtkYetULPszMaK2Jksv".to_string(),
            pubkey: None,
            description: "Puzzle #74 - Range 2^73 to 2^74 (SOLVED, pubkey kapali)".to_string(),
        },
        Puzzle {
            id: 135,
            power: 135,
            address: "16RGFo6hjq9ym6Pj7N5H7L1NR1rVPJyw2v".to_string(),
            pubkey: Some("02145d2611c823a396ef6712ce0f712f09b9b4f3135e3e0aa3230fb9b6d08d1e16".to_string()),
            description: "Puzzle #135 - Range 2^134 to 2^135 (pubkey ACIK)".to_string(),
        },
        Puzzle {
            id: 140,
            power: 140,
            address: "1QKBaU6WAeycb3DbKbLBkX7vJiaS8r42Xo".to_string(),
            pubkey: Some("031f6a332d3c5c4f2de2378c012f429cd109ba07d69690c6c701b6bb87860d6640".to_string()),
            description: "Puzzle #140 - Range 2^139 to 2^140 (pubkey ACIK)".to_string(),
        },
        Puzzle {
            id: 145,
            power: 145,
            address: "19GpszRNUej5yYqxXoLnbZWKew3KdVLkXg".to_string(),
            pubkey: Some("03afdda497369e219a2c1c369954a930e4d3740968e5e4352475bcffce3140dae5".to_string()),
            description: "Puzzle #145 - Range 2^144 to 2^145 (pubkey ACIK)".to_string(),
        },
        Puzzle {
            id: 150,
            power: 150,
            address: "1MUJSJYtGPVGkBCTqGspnxyHahpt5Te8jy".to_string(),
            pubkey: Some("03137807790ea7dc6e97901c2bc87411f45ed74a5629315c4e4b03a0a102250c49".to_string()),
            description: "Puzzle #150 - Range 2^149 to 2^150 (pubkey ACIK)".to_string(),
        },
        Puzzle {
            id: 155,
            power: 155,
            address: "1AoeP37TmHdFh8uN72fu9AqgtLrUwcv2wJ".to_string(),
            pubkey: Some("035cd1854cae45391ca4ec428cc7e6c7d9984424b954209a8eea197b9e364c05f6".to_string()),
            description: "Puzzle #155 - Range 2^154 to 2^155 (pubkey ACIK)".to_string(),
        },
        Puzzle {
            id: 160,
            power: 160,
            address: "1NBC8uXJy1GiJ6drkiZa1WuKn51ps7EPTv".to_string(),
            pubkey: Some("02e0a8b039282faf6fe0fd769cfbc4b6b4cf8758ba68220eac420e32b91ddfa673".to_string()),
            description: "Puzzle #160 - Range 2^159 to 2^160 (pubkey ACIK)".to_string(),
        },
    ]
}
