use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Puzzle {
    pub id: u32,
    pub power: u32,
    pub address: String,
    pub description: String,
}

pub fn get_active_puzzles() -> Vec<Puzzle> {
    vec![
        Puzzle {
            id: 66,
            power: 66,
            address: "13zb1hQbWVsc2S7ZTZnP2G4undNNpdh5so".to_string(),
            description: "Puzzle #66 - Range 2^65 to 2^66".to_string(),
        },
        Puzzle {
            id: 67,
            power: 67,
            address: "1BY8GQbnueYofwSuFAT3USAhGjPrkxDdW9".to_string(),
            description: "Puzzle #67 - Range 2^66 to 2^67".to_string(),
        },
        Puzzle {
            id: 68,
            power: 68,
            address: "1MVDYgVaSN6iKKEsbzRUAYFrYJadLYZvvZ".to_string(),
            description: "Puzzle #68 - Range 2^67 to 2^68".to_string(),
        },
        Puzzle {
            id: 69,
            power: 69,
            address: "19vkiEajfhuZBBbsHZ8Jd2KYz5q3sZ5G9g".to_string(),
            description: "Puzzle #69 - Range 2^68 to 2^69".to_string(),
        },
        Puzzle {
            id: 70,
            power: 70,
            address: "1PWo3JeB9jrGwfHDNpdGK54CRas7fsVzXU".to_string(),
            description: "Puzzle #70 - Range 2^69 to 2^70".to_string(),
        },
        Puzzle {
            id: 71,
            power: 71,
            address: "1JhCQPhF6T4shR8ojJ8Yr5vgc4N6MxxHQz".to_string(),
            description: "Puzzle #71 - Range 2^70 to 2^71".to_string(),
        },
        Puzzle {
            id: 72,
            power: 72,
            address: "1ACNaY7JquJ7Z3WgkRUtDCEmF23DGZ1Biw".to_string(),
            description: "Puzzle #72 - Range 2^71 to 2^72".to_string(),
        },
        Puzzle {
            id: 73,
            power: 73,
            address: "1Fz63cJc2TwZAi1SR1tGV1tmCJeTqGCWe9".to_string(),
            description: "Puzzle #73 - Range 2^72 to 2^73".to_string(),
        },
        Puzzle {
            id: 74,
            power: 74,
            address: "1FWGcVDK3JGzCC3WtkYetULPszMaK2Jksv".to_string(),
            description: "Puzzle #74 - Range 2^73 to 2^74 (SOLVED)".to_string(),
        },
    ]
}
