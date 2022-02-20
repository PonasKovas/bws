// impl Deserializable for NbtCompound {
//     fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
//         let string = String::from_reader(input)?;

//         match string.as_str() {
//             "minecraft:ask_server" => Ok(Self::AskServer),
//             "minecraft:all_recipes" => Ok(Self::AllRecipes),
//             "minecraft:available_sounds" => Ok(Self::AvailableSounds),
//             "minecraft:summonable_entities" => Ok(Self::SummonableEntities),
//             _ => Err(std::io::Error::new(
//                 ErrorKind::InvalidData,
//                 "Invalid suggestion type",
//             )),
//         }
//     }
// }
