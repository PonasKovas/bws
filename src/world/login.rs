use crate::chat_parse;
use crate::datatypes::*;
use crate::internal_communication::{SHBound, SHSender};
use crate::packets::{ClientBound, TitleAction};
use crate::world::World;
use crate::GLOBAL_STATE;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use log::{debug, error, info, warn};
use sha2::{Digest, Sha256};
use slab::Slab;
use std::collections::HashMap;
use std::env::Vars;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;

const ACCOUNTS_FILE: &str = "accounts.bwsdata";

pub struct LoginWorld {
    players: HashMap<usize, (String, SHSender, Option<String>)>, // username and SHSender, and the password hash, if registered
    accounts: HashMap<String, String>,
    login_message: Chat,
    register_message: Chat,
}

impl World for LoginWorld {
    fn get_world_name(&self) -> &str {
        "authentication"
    }
    fn is_fixed_time(&self) -> Option<i64> {
        Some(18000)
    }
    fn add_player(&mut self, id: usize) -> Result<()> {
        let lock = futures::executor::block_on(GLOBAL_STATE.players.lock());
        let sh_sender = lock
            .get(id)
            .context("tried to add non-existing player")?
            .sh_sender
            .clone();
        let username = lock
            .get(id)
            .context("tried to add non-existing player")?
            .username
            .clone();
        drop(lock);

        let mut dimension = nbt::Blob::new();

        // rustfmt makes this block reaaally fat and ugly and disgusting oh my god
        #[rustfmt::skip]
        {
            use nbt::Value::{Byte, Float, Int, Long, String as NbtString};

            dimension.insert("piglin_safe".to_string(), Byte(0)).unwrap();
            dimension.insert("natural".to_string(), Byte(1)).unwrap();
            dimension.insert("ambient_light".to_string(), Float(1.0)).unwrap();
            if let Some(time) = self.is_fixed_time() {
                dimension.insert("fixed_time".to_string(), Long(time)).unwrap();
            }
            dimension.insert("infiniburn".to_string(), NbtString("".to_string())).unwrap();
            dimension.insert("respawn_anchor_works".to_string(), Byte(1)).unwrap();
            dimension.insert("has_skylight".to_string(), Byte(1)).unwrap();
            dimension.insert("bed_works".to_string(), Byte(0)).unwrap();
            dimension.insert("effects".to_string(), NbtString("minecraft:overworld".to_string())).unwrap();
            dimension.insert("has_raids".to_string(), Byte(0)).unwrap();
            dimension.insert("logical_height".to_string(), Int(256)).unwrap();
            dimension.insert("coordinate_scale".to_string(), Float(1.0)).unwrap();
            dimension.insert("ultrawarm".to_string(), Byte(0)).unwrap();
            dimension.insert("has_ceiling".to_string(), Byte(0)).unwrap();
        };

        let packet = ClientBound::JoinGame(
            id as i32,
            false,
            3,
            -1,
            vec![self.get_world_name().to_string()],
            dimension,
            self.get_world_name().to_string(),
            0,
            VarInt(20),
            VarInt(8),
            false,
            false,
            false,
            true,
        );
        sh_sender.send(SHBound::Packet(packet))?;

        sh_sender.send(SHBound::Packet(ClientBound::PlayerPositionAndLook(
            0.0,
            0.0,
            0.0,
            0.0,
            -20.0,
            0,
            VarInt(0),
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::SetBrand("BWS".to_string())))?;

        // oh my god.
        sh_sender.send(SHBound::Packet(ClientBound::Tags(
            [
                (
                    "minecraft:enderman_holdable",
                    vec![
                        119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 131, 130, 8, 9, 10,
                        11, 28, 29, 30, 132, 133, 137, 187, 188, 192, 202, 244, 253, 697, 696, 703,
                        688, 687, 690,
                    ],
                ),
                ("minecraft:soul_fire_base_blocks", vec![194, 195]),
                ("minecraft:campfires", vec![680, 681]),
                (
                    "minecraft:banners",
                    vec![
                        416, 417, 418, 419, 420, 421, 422, 423, 424, 425, 426, 427, 428, 429, 430,
                        431, 432, 433, 434, 435, 436, 437, 438, 439, 440, 441, 442, 443, 444, 445,
                        446, 447,
                    ],
                ),
                ("minecraft:infiniburn_nether", vec![193, 503]),
                ("minecraft:infiniburn_overworld", vec![193, 503]),
                (
                    "minecraft:flower_pots",
                    vec![
                        281, 290, 291, 292, 293, 294, 295, 296, 297, 298, 289, 282, 283, 284, 285,
                        286, 287, 302, 303, 304, 288, 305, 299, 300, 301, 623, 738, 739, 740, 741,
                    ],
                ),
                (
                    "minecraft:wooden_fences",
                    vec![191, 483, 484, 480, 481, 482, 710, 711],
                ),
                ("minecraft:piglin_repellents", vec![144, 198, 679, 199, 681]),
                (
                    "minecraft:wall_post_override",
                    vec![
                        141, 198, 181, 272, 155, 156, 157, 158, 159, 160, 722, 723, 165, 166, 167,
                        168, 169, 170, 724, 725, 416, 417, 418, 419, 420, 421, 422, 423, 424, 425,
                        426, 427, 428, 429, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 440,
                        441, 442, 443, 444, 445, 446, 447, 330, 331, 174, 175, 176, 177, 178, 179,
                        708, 709, 172, 757,
                    ],
                ),
                (
                    "minecraft:wooden_slabs",
                    vec![452, 453, 454, 455, 456, 457, 706, 707],
                ),
                ("minecraft:portals", vec![201, 262, 499]),
                (
                    "minecraft:small_flowers",
                    vec![
                        119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 131, 130,
                    ],
                ),
                (
                    "minecraft:bamboo_plantable_on",
                    vec![28, 29, 622, 621, 30, 9, 8, 11, 10, 253],
                ),
                (
                    "minecraft:wooden_trapdoors",
                    vec![226, 224, 227, 225, 222, 223, 712, 713],
                ),
                (
                    "minecraft:pressure_plates",
                    vec![330, 331, 174, 175, 176, 177, 178, 179, 708, 709, 172, 757],
                ),
                ("minecraft:jungle_logs", vec![38, 50, 43, 56]),
                (
                    "minecraft:wooden_stairs",
                    vec![146, 274, 275, 276, 375, 376, 716, 717],
                ),
                ("minecraft:spruce_logs", vec![36, 48, 41, 54]),
                (
                    "minecraft:signs",
                    vec![
                        155, 156, 157, 158, 159, 160, 722, 723, 165, 166, 167, 168, 169, 170, 724,
                        725,
                    ],
                ),
                (
                    "minecraft:carpets",
                    vec![
                        391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405,
                        406,
                    ],
                ),
                ("minecraft:base_stone_overworld", vec![1, 2, 4, 6]),
                (
                    "minecraft:wool",
                    vec![
                        102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
                        117,
                    ],
                ),
                (
                    "minecraft:wooden_buttons",
                    vec![308, 309, 310, 311, 312, 313, 718, 719],
                ),
                ("minecraft:wither_summon_base_blocks", vec![194, 195]),
                (
                    "minecraft:stairs",
                    vec![
                        146, 274, 275, 276, 375, 376, 716, 717, 164, 268, 257, 252, 251, 495, 340,
                        451, 384, 383, 385, 627, 628, 629, 630, 631, 632, 633, 634, 635, 636, 637,
                        638, 639, 640, 744, 752, 755,
                    ],
                ),
                (
                    "minecraft:logs",
                    vec![
                        40, 52, 45, 58, 35, 47, 46, 53, 39, 51, 44, 57, 37, 49, 42, 55, 38, 50, 43,
                        56, 36, 48, 41, 54, 692, 693, 694, 695, 683, 684, 685, 686,
                    ],
                ),
                ("minecraft:stone_bricks", vec![228, 229, 230, 231]),
                ("minecraft:hoglin_repellents", vec![688, 739, 201, 737]),
                ("minecraft:fire", vec![143, 144]),
                ("minecraft:beehives", vec![730, 731]),
                ("minecraft:ice", vec![185, 409, 619, 502]),
                ("minecraft:base_stone_nether", vec![193, 196, 743]),
                (
                    "minecraft:dragon_immune",
                    vec![
                        378, 25, 262, 263, 499, 277, 500, 501, 726, 727, 118, 140, 736, 264, 241,
                        737,
                    ],
                ),
                ("minecraft:crops", vec![497, 306, 307, 152, 248, 247]),
                (
                    "minecraft:wall_signs",
                    vec![165, 166, 167, 168, 169, 170, 724, 725],
                ),
                (
                    "minecraft:slabs",
                    vec![
                        452, 453, 454, 455, 456, 457, 706, 707, 458, 459, 465, 460, 470, 467, 468,
                        464, 463, 466, 462, 386, 387, 388, 641, 642, 643, 644, 645, 646, 647, 648,
                        649, 650, 651, 652, 653, 461, 469, 746, 751, 756,
                    ],
                ),
                ("minecraft:valid_spawn", vec![8, 11]),
                ("minecraft:mushroom_grow_block", vec![253, 11, 696, 687]),
                (
                    "minecraft:guarded_by_piglins",
                    vec![
                        134, 668, 147, 270, 754, 329, 509, 525, 521, 522, 519, 517, 523, 513, 518,
                        515, 512, 511, 516, 520, 524, 510, 514, 31, 34,
                    ],
                ),
                (
                    "minecraft:wooden_doors",
                    vec![161, 485, 486, 487, 488, 489, 720, 721],
                ),
                ("minecraft:warped_stems", vec![683, 684, 685, 686]),
                (
                    "minecraft:standing_signs",
                    vec![155, 156, 157, 158, 159, 160, 722, 723],
                ),
                ("minecraft:infiniburn_end", vec![193, 503, 25]),
                (
                    "minecraft:trapdoors",
                    vec![226, 224, 227, 225, 222, 223, 712, 713, 379],
                ),
                ("minecraft:crimson_stems", vec![692, 693, 694, 695]),
                (
                    "minecraft:buttons",
                    vec![308, 309, 310, 311, 312, 313, 718, 719, 183, 758],
                ),
                (
                    "minecraft:flowers",
                    vec![
                        119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 131, 130, 410, 411,
                        413, 412,
                    ],
                ),
                (
                    "minecraft:corals",
                    vec![593, 594, 595, 596, 597, 603, 604, 605, 606, 607],
                ),
                (
                    "minecraft:prevent_mob_spawning_inside",
                    vec![163, 91, 92, 341],
                ),
                ("minecraft:wart_blocks", vec![504, 689]),
                (
                    "minecraft:climbable",
                    vec![162, 249, 666, 699, 700, 701, 702],
                ),
                ("minecraft:planks", vec![13, 14, 15, 16, 17, 18, 704, 705]),
                ("minecraft:soul_speed_blocks", vec![194, 195]),
                ("minecraft:dark_oak_logs", vec![40, 52, 45, 58]),
                ("minecraft:rails", vec![163, 91, 92, 341]),
                ("minecraft:coral_plants", vec![593, 594, 595, 596, 597]),
                (
                    "minecraft:non_flammable_wood",
                    vec![
                        683, 684, 685, 686, 692, 693, 694, 695, 704, 705, 706, 707, 708, 709, 710,
                        711, 712, 713, 714, 715, 716, 717, 718, 719, 720, 721, 722, 723, 724, 725,
                    ],
                ),
                ("minecraft:leaves", vec![62, 59, 60, 64, 63, 61]),
                (
                    "minecraft:walls",
                    vec![
                        279, 280, 654, 655, 656, 657, 658, 659, 660, 661, 662, 663, 664, 665, 745,
                        753, 759,
                    ],
                ),
                ("minecraft:coral_blocks", vec![583, 584, 585, 586, 587]),
                ("minecraft:strider_warm_blocks", vec![27]),
                (
                    "minecraft:beacon_base_blocks",
                    vec![734, 273, 150, 134, 135],
                ),
                (
                    "minecraft:fence_gates",
                    vec![478, 476, 479, 477, 250, 475, 714, 715],
                ),
                (
                    "minecraft:shulker_boxes",
                    vec![
                        509, 525, 521, 522, 519, 517, 523, 513, 518, 515, 512, 511, 516, 520, 524,
                        510, 514,
                    ],
                ),
                (
                    "minecraft:bee_growables",
                    vec![497, 306, 307, 152, 248, 247, 682],
                ),
                (
                    "minecraft:wooden_pressure_plates",
                    vec![174, 175, 176, 177, 178, 179, 708, 709],
                ),
                (
                    "minecraft:wither_immune",
                    vec![378, 25, 262, 263, 499, 277, 500, 501, 726, 727, 118],
                ),
                ("minecraft:acacia_logs", vec![39, 51, 44, 57]),
                ("minecraft:anvil", vec![326, 327, 328]),
                ("minecraft:tall_flowers", vec![410, 411, 413, 412]),
                ("minecraft:birch_logs", vec![37, 49, 42, 55]),
                ("minecraft:wall_corals", vec![613, 614, 615, 616, 617]),
                (
                    "minecraft:underwater_bonemeals",
                    vec![
                        98, 593, 594, 595, 596, 597, 603, 604, 605, 606, 607, 613, 614, 615, 616,
                        617,
                    ],
                ),
                ("minecraft:stone_pressure_plates", vec![172, 757]),
                (
                    "minecraft:impermeable",
                    vec![
                        67, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219,
                        220, 221,
                    ],
                ),
                ("minecraft:sand", vec![28, 29]),
                ("minecraft:nylium", vec![696, 687]),
                ("minecraft:gold_ores", vec![31, 34]),
                (
                    "minecraft:fences",
                    vec![191, 483, 484, 480, 481, 482, 710, 711, 256],
                ),
                (
                    "minecraft:logs_that_burn",
                    vec![
                        40, 52, 45, 58, 35, 47, 46, 53, 39, 51, 44, 57, 37, 49, 42, 55, 38, 50, 43,
                        56, 36, 48, 41, 54,
                    ],
                ),
                ("minecraft:saplings", vec![19, 20, 21, 22, 23, 24]),
                (
                    "minecraft:beds",
                    vec![
                        89, 90, 86, 87, 84, 82, 88, 78, 83, 80, 77, 76, 81, 85, 75, 79,
                    ],
                ),
                (
                    "minecraft:unstable_bottom_center",
                    vec![478, 476, 479, 477, 250, 475, 714, 715],
                ),
                ("minecraft:oak_logs", vec![35, 47, 46, 53]),
                (
                    "minecraft:doors",
                    vec![161, 485, 486, 487, 488, 489, 720, 721, 173],
                ),
            ]
            .map(|(name, entries)| {
                (
                    name.to_string(),
                    entries.iter().map(|entry| VarInt(*entry)).collect(),
                )
            })
            .to_vec(),
            [
                ("minecraft:soul_fire_base_blocks", vec![219, 220]),
                (
                    "minecraft:banners",
                    vec![
                        870, 871, 872, 873, 874, 875, 876, 877, 878, 879, 880, 881, 882, 883, 884,
                        885,
                    ],
                ),
                ("minecraft:stone_crafting_materials", vec![14, 963]),
                (
                    "minecraft:wooden_fences",
                    vec![208, 212, 213, 209, 210, 211, 214, 215],
                ),
                ("minecraft:piglin_repellents", vec![223, 947, 950]),
                (
                    "minecraft:beacon_payment_items",
                    vec![581, 827, 578, 580, 579],
                ),
                (
                    "minecraft:wooden_slabs",
                    vec![138, 139, 140, 141, 142, 143, 144, 145],
                ),
                (
                    "minecraft:small_flowers",
                    vec![
                        111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123,
                    ],
                ),
                (
                    "minecraft:wooden_trapdoors",
                    vec![230, 228, 231, 229, 226, 227, 232, 233],
                ),
                ("minecraft:jungle_logs", vec![40, 64, 48, 56]),
                ("minecraft:lectern_books", vec![826, 825]),
                (
                    "minecraft:wooden_stairs",
                    vec![179, 280, 281, 282, 369, 370, 283, 284],
                ),
                ("minecraft:spruce_logs", vec![38, 62, 46, 54]),
                (
                    "minecraft:signs",
                    vec![652, 653, 654, 656, 655, 657, 658, 659],
                ),
                (
                    "minecraft:carpets",
                    vec![
                        350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364,
                        365,
                    ],
                ),
                (
                    "minecraft:wool",
                    vec![
                        95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110,
                    ],
                ),
                (
                    "minecraft:wooden_buttons",
                    vec![305, 306, 307, 308, 309, 310, 311, 312],
                ),
                (
                    "minecraft:stairs",
                    vec![
                        179, 280, 281, 282, 369, 370, 283, 284, 188, 275, 268, 261, 260, 177, 328,
                        421, 415, 414, 416, 529, 530, 531, 532, 533, 534, 535, 536, 537, 538, 539,
                        540, 541, 542, 965, 973, 969,
                    ],
                ),
                ("minecraft:fishes", vec![687, 691, 688, 692, 690, 689]),
                (
                    "minecraft:logs",
                    vec![
                        42, 66, 50, 58, 37, 61, 45, 53, 41, 65, 49, 57, 39, 63, 47, 55, 40, 64, 48,
                        56, 38, 62, 46, 54, 43, 51, 67, 59, 44, 52, 68, 60,
                    ],
                ),
                ("minecraft:stone_bricks", vec![240, 241, 242, 243]),
                (
                    "minecraft:creeper_drop_music_discs",
                    vec![909, 910, 911, 912, 913, 914, 915, 916, 917, 918, 919, 920],
                ),
                ("minecraft:arrows", vec![575, 895, 894]),
                (
                    "minecraft:slabs",
                    vec![
                        138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 153, 148, 158, 155, 156,
                        152, 151, 154, 150, 159, 160, 161, 543, 544, 545, 546, 547, 548, 549, 550,
                        551, 552, 553, 554, 555, 149, 157, 964, 972, 968,
                    ],
                ),
                (
                    "minecraft:wooden_doors",
                    vec![558, 559, 560, 561, 562, 563, 564, 565],
                ),
                ("minecraft:warped_stems", vec![44, 52, 68, 60]),
                (
                    "minecraft:trapdoors",
                    vec![230, 228, 231, 229, 226, 227, 232, 233, 348],
                ),
                ("minecraft:crimson_stems", vec![43, 51, 67, 59]),
                (
                    "minecraft:buttons",
                    vec![305, 306, 307, 308, 309, 310, 311, 312, 304, 313],
                ),
                (
                    "minecraft:flowers",
                    vec![
                        111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 373, 374,
                        376, 375,
                    ],
                ),
                ("minecraft:stone_tool_materials", vec![14, 963]),
                ("minecraft:planks", vec![15, 16, 17, 18, 19, 20, 21, 22]),
                ("minecraft:boats", vec![667, 899, 900, 901, 902, 903]),
                ("minecraft:dark_oak_logs", vec![42, 66, 50, 58]),
                ("minecraft:rails", vec![187, 85, 86, 329]),
                (
                    "minecraft:non_flammable_wood",
                    vec![
                        44, 52, 68, 60, 43, 51, 67, 59, 21, 22, 144, 145, 197, 198, 214, 215, 232,
                        233, 258, 259, 283, 284, 311, 312, 564, 565, 658, 659,
                    ],
                ),
                ("minecraft:leaves", vec![72, 69, 70, 74, 73, 71]),
                (
                    "minecraft:walls",
                    vec![
                        287, 288, 289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299, 300, 301,
                        303, 302,
                    ],
                ),
                ("minecraft:coals", vec![576, 577]),
                (
                    "minecraft:wooden_pressure_plates",
                    vec![191, 192, 193, 194, 195, 196, 197, 198],
                ),
                ("minecraft:acacia_logs", vec![41, 65, 49, 57]),
                (
                    "minecraft:music_discs",
                    vec![
                        909, 910, 911, 912, 913, 914, 915, 916, 917, 918, 919, 920, 921,
                    ],
                ),
                ("minecraft:anvil", vec![314, 315, 316]),
                (
                    "minecraft:piglin_loved",
                    vec![
                        33, 36, 136, 966, 318, 580, 945, 685, 835, 758, 650, 651, 638, 639, 640,
                        641, 862, 593, 595, 594, 596, 597,
                    ],
                ),
                ("minecraft:tall_flowers", vec![373, 374, 376, 375]),
                ("minecraft:birch_logs", vec![39, 63, 47, 55]),
                ("minecraft:sand", vec![30, 31]),
                ("minecraft:gold_ores", vec![33, 36]),
                (
                    "minecraft:fences",
                    vec![208, 212, 213, 209, 210, 211, 214, 215, 267],
                ),
                (
                    "minecraft:logs_that_burn",
                    vec![
                        42, 66, 50, 58, 37, 61, 45, 53, 41, 65, 49, 57, 39, 63, 47, 55, 40, 64, 48,
                        56, 38, 62, 46, 54,
                    ],
                ),
                ("minecraft:saplings", vec![23, 24, 25, 26, 27, 28]),
                (
                    "minecraft:beds",
                    vec![
                        730, 731, 727, 728, 725, 723, 729, 719, 724, 721, 718, 717, 722, 726, 716,
                        720,
                    ],
                ),
                ("minecraft:oak_logs", vec![37, 61, 45, 53]),
                (
                    "minecraft:doors",
                    vec![558, 559, 560, 561, 562, 563, 564, 565, 557],
                ),
            ]
            .map(|(name, entries)| {
                (
                    name.to_string(),
                    entries.iter().map(|entry| VarInt(*entry)).collect(),
                )
            })
            .to_vec(),
            [
                ("minecraft:lava", vec![4, 3]),
                ("minecraft:water", vec![2, 1]),
            ]
            .map(|(name, entries)| {
                (
                    name.to_string(),
                    entries.iter().map(|entry| VarInt(*entry)).collect(),
                )
            })
            .to_vec(),
            [
                ("minecraft:beehive_inhabitors", vec![4]),
                (
                    "minecraft:impact_projectiles",
                    vec![2, 79, 78, 39, 76, 84, 88, 15, 99],
                ),
                ("minecraft:skeletons", vec![73, 82, 98]),
                ("minecraft:raiders", vec![22, 62, 67, 94, 35, 96]),
                ("minecraft:arrows", vec![2, 79]),
            ]
            .map(|(name, entries)| {
                (
                    name.to_string(),
                    entries.iter().map(|entry| VarInt(*entry)).collect(),
                )
            })
            .to_vec(),
        )))?;

        let password = self.accounts.get(&username);

        // declare commands
        sh_sender.send(SHBound::Packet(ClientBound::DeclareCommands(
            if password.is_some() {
                vec![
                    CommandNode::Root(vec![VarInt(1)]),
                    CommandNode::Literal(false, vec![VarInt(2)], None, "login".to_string()),
                    CommandNode::Argument(
                        true,
                        Vec::new(),
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                ]
            } else {
                vec![
                    CommandNode::Root(vec![VarInt(1)]),
                    CommandNode::Literal(false, vec![VarInt(2)], None, "register".to_string()),
                    CommandNode::Argument(
                        false,
                        vec![VarInt(3)],
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                    CommandNode::Argument(
                        true,
                        Vec::new(),
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                ]
            },
            VarInt(0),
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(TitleAction::Reset)))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(TitleAction::SetTitle(
            chat_parse("§bWelcome to §d§lBWS§r§b!"),
        ))))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(
            TitleAction::SetDisplayTime(15, 60, 15),
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::EntitySoundEffect(
            VarInt(482),
            VarInt(0),         // MASTER category
            VarInt(id as i32), // player
            1.0,
            1.0,
        )))?;

        // add the player
        self.players
            .insert(id, (username, sh_sender, password.cloned()));

        Ok(())
    }
    fn remove_player(&mut self, id: usize) {
        self.players.remove(&id);
    }
    fn sh_send(&self, id: usize, message: SHBound) -> Result<()> {
        self.players
            .get(&id)
            .context("No player with given ID in this world")?
            .1
            .send(message)?;
        Ok(())
    }
    fn tick(&mut self, counter: u64) {
        if counter % 20 == 0 {
            for (id, player) in &self.players {
                let subtitle = if self.accounts.contains_key(&player.0) {
                    &self.login_message
                } else {
                    &self.register_message
                };
                if let Err(e) = self.sh_send(
                    *id,
                    SHBound::Packet(ClientBound::Title(TitleAction::SetActionBar(
                        subtitle.clone(),
                    ))),
                ) {
                    debug!("Couldn't send packet to client: {}", e);
                }
            }
        }
    }
    fn chat(&mut self, id: usize, message: String) -> Result<()> {
        match &self.players.get(&id).context("No player with given ID")?.2 {
            Some(password_hash) => {
                if message.starts_with("/login ") {
                    let mut iterator = message.split(' ');
                    if let Some(password) = iterator.nth(1) {
                        let hash = format!("{:x}", Sha256::digest(password.as_bytes()));
                        if *password_hash == hash {
                            self.sh_send(id, SHBound::ChangeWorld(GLOBAL_STATE.w_lobby.clone()))?;
                        } else {
                            self.tell(id, "§4§lIncorrect password!".to_string())?;
                        }
                        return Ok(());
                    }
                }
            }
            None => {
                if message.starts_with("/register ") {
                    let mut iterator = message.split(' ');
                    if let Some(first_password) = iterator.nth(1) {
                        if let Some(second_password) = iterator.next() {
                            if first_password != second_password {
                                self.tell(id, "§cThe passwords do not match, try again.")?;
                                return Ok(());
                            }

                            // register the gentleman
                            self.accounts.insert(
                                self.username(id)?.to_string(),
                                format!("{:x}", Sha256::digest(first_password.as_bytes())),
                            );
                            self.save_accounts()?;

                            self.sh_send(id, SHBound::ChangeWorld(GLOBAL_STATE.w_lobby.clone()))?;

                            return Ok(());
                        }
                    }
                }
            }
        }

        if message.starts_with("/") {
            self.tell(id, "§cInvalid command.")?;
        }
        Ok(())
    }
    fn username(&self, id: usize) -> Result<&str> {
        Ok(&self
            .players
            .get(&id)
            .context("No player with given ID in this world")?
            .0)
    }
}

pub fn new() -> Result<LoginWorld> {
    // read the accounts data
    let mut accounts = HashMap::new();
    if Path::new(ACCOUNTS_FILE).exists() {
        // read the data
        let f = File::open(ACCOUNTS_FILE).context(format!("Failed to open {}.", ACCOUNTS_FILE))?;

        let file = BufReader::new(f);
        for line in file.lines() {
            let line = line.context(format!("Error reading {}.", ACCOUNTS_FILE))?;
            let mut iterator = line.split(' ');

            let username = iterator
                .next()
                .context(format!("Incorrect {} format.", ACCOUNTS_FILE))?;
            let password_hash = iterator
                .next()
                .context(format!("Incorrect {} format.", ACCOUNTS_FILE))?;

            accounts.insert(username.to_string(), password_hash.to_string());
        }
    } else {
        // create the file
        File::create(ACCOUNTS_FILE)?;
    }

    Ok(LoginWorld {
        players: HashMap::new(),
        accounts,
        login_message: chat_parse("§aType §6/login §3<password> §ato continue"),
        register_message: chat_parse(
            "§aType §6/register §3<password> <password again> §ato continue",
        ),
    })
}

impl LoginWorld {
    pub fn tell<T: AsRef<str>>(&self, id: usize, message: T) -> Result<()> {
        self.sh_send(
            id,
            SHBound::Packet(ClientBound::ChatMessage(chat_parse(message), 1)),
        )?;
        Ok(())
    }
    pub fn save_accounts(&self) -> Result<()> {
        let mut f = File::create(ACCOUNTS_FILE)?;

        for account in &self.accounts {
            // I wish to apologize for the readability of the following statement
            #[rustfmt::skip]
            f.write_all(account.0.as_bytes()).and(
                f.write_all(b" ").and(
                    f.write_all(account.1.as_bytes()).and(
                        f.write_all(b"\n")
                    )
                ),
            ).context(format!("Couldn't write to {}", ACCOUNTS_FILE))?;
        }

        Ok(())
    }
}
