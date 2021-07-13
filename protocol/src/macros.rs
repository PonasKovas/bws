/// A macro that expands to a [`CommandsBuilder`][crate::commands_builder::CommandsBuilder] instance ready to be built.
///
/// Syntax:
///
/// ```
/// command!(([X] "NAME", <literal|argument (TYPE: OPTIONS) [suggestions=SUGGESTIONS]> => [...]), ...);
/// ```
///
/// * `X` means the node is executable (can be present or not)
/// * `"NAME"` is the name of node
/// * `literal`/`argument` is the type of the node
/// * `TYPE` (only when the node is an argument) is the parser type for the node (variant of [`datatypes::Parser`][crate::datatypes::Parser])
/// * `OPTIONS` is the options of the parser. Syntax depends on the parser, see full list below.
/// * `SUGGESTIONS` is suggestions type for the node, can be ommited if no suggestions needed. Must be a variant of [`datatypes::SuggestionsType`][crate::datatypes::SuggestionsType]
///
/// Example:
///
/// ```rust
/// let packet: PlayClientBound = command!(
///     (X "simple_command", literal => []),
///     ("number", literal => [
///         (X "value", argument (Integer: Some(0), None) => [])
///     ]),
///     ("string", literal => [
///         (X "value", argument (String: SingleWord) suggestions=AskServer => [])
///     ]),
///     ("bool", literal => [
///         (X "value", argument (Bool) => [])
///     ]),
/// ).build();
/// ```
///
/// ---
///
/// ## Options for the parser
///
/// | Parser                                         | Options                                                             |
/// |------------------------------------------------|---------------------------------------------------------------------|
/// | [`String`][crate::datatypes::Parser::String]   | [`datatypes::StringParserType`][crate::datatypes::StringParserType] |
/// | [`Integer`][crate::datatypes::Parser::Integer] | `Option<i32>, Option<i32>` (minimum and maximum integers)           |
/// | [`Bool`][crate::datatypes::Parser::Bool]       | None                                                                |
///
#[macro_export]
macro_rules! command {
    ($($node:tt),* $(,)?) => {
        {
            let mut res = $crate::commands_builder::CommandsBuilder::new();
            $(res.add($crate::single_node!($node));)*
            res
        }
    };
    // create a DeclareCommands with no commands
    () => {
        $crate::packets::PlayClientBound::DeclareCommands {
            nodes: vec![$crate::datatypes::CommandNode::Root {
                children: std::vec::Vec::new(),
            }],
            root: $crate::datatypes::VarInt(0),
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! single_node {
    (($name:expr, literal => [$($node:tt),* $(,)?])) => {
        {
            let mut res = $crate::commands_builder::BuilderNode::Literal {
                name: $name.into(),
                executable: false,
                children: Vec::new(),
            };

            $(res.add($crate::single_node!($node));)*

            res
        }
    };
    ((X $name:expr, literal => [$($node:tt),*])) => {
        {
            let mut res = $crate::commands_builder::BuilderNode::Literal {
                name: $name.into(),
                executable: true,
                children: Vec::new(),
            };

            $(res.add($crate::single_node!($node));)*

            res
        }
    };
    (($name:expr, argument $parser:tt => [$($node:tt),*])) => {
        {
            let mut res = $crate::commands_builder::BuilderNode::Argument {
                name: $name.into(),
                executable: false,
                parser: $crate::handle_parser!($parser),
                suggestions: None,
                children: Vec::new(),
            };

            $(res.add($crate::single_node!($node));)*

            res
        }
    };
    ((X $name:expr, argument $parser:tt => [$($node:tt),*])) => {
        {
            let mut res = $crate::commands_builder::BuilderNode::Argument {
                name: $name.into(),
                executable: true,
                parser: $crate::handle_parser!($parser),
                suggestions: None,
                children: Vec::new(),
            };

            $(res.add($crate::single_node!($node));)*

            res
        }
    };
    (($name:expr, argument $parser:tt suggestions=$suggestions:ident => [$($node:tt),*])) => {
        {
            let mut res = $crate::commands_builder::BuilderNode::Argument {
                name: $name.into(),
                executable: false,
                parser: $crate::handle_parser!($parser),
                suggestions: Some($crate::datatypes::SuggestionsType::$suggestions),
                children: Vec::new(),
            };

            $(res.add($crate::single_node!($node));)*

            res
        }
    };
    ((X $name:expr, argument $parser:tt suggestions=$suggestions:ident => [$($node:tt),*])) => {
        {
            let mut res = $crate::commands_builder::BuilderNode::Argument {
                name: $name.into(),
                executable: true,
                parser: $crate::handle_parser!($parser),
                suggestions: Some($crate::datatypes::SuggestionsType::$suggestions),
                children: Vec::new(),
            };

            $(res.add($crate::single_node!($node));)*

            res
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! handle_parser {
    ((String: $type:ident)) => {
        $crate::datatypes::Parser::String($crate::datatypes::StringParserType::$type)
    };
    ((Integer: $min:expr, $max:expr)) => {
        $crate::datatypes::Parser::Integer($crate::datatypes::IntegerParserOptions {
            min: $min,
            max: $max,
        })
    };
    ((Bool)) => {
        $crate::datatypes::Parser::Bool
    };
}
