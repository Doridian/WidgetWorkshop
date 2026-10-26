//! The gadget classes: each one's tag, numbers and behaviour. The factory
//! (`0x41f194`) creates a class from its tag; the numbers come from each
//! class's init function (the sheet it loads with `0x422940`, the knot
//! table with `0x41db6c`, the gadget id it stores at `+0x28`).

mod common;
pub mod displays;
pub mod inputs;
pub mod labels;
mod logic;
mod numbers;
mod science;
pub mod sound_time;
mod x87;

use crate::gadget::{Behavior, Tag};

/// A gadget class.
pub struct Class {
    pub tag: Tag,
    /// The original's class name.
    pub name: &'static str,
    /// Its gadget id (`+0x28`): settings dialog, Doc text, info. Not
    /// always its pictures' and knots' number.
    pub gadget_id: u16,
    /// Its picture sheet (type 1004 in `WWGADGET.DLL`).
    pub sheet: u32,
    /// Its knot table (type 1014), where it isn't the sheet's number.
    pub knots: u32,
    pub make: fn() -> Box<dyn Behavior>,
}

macro_rules! classes {
    ($($tag:literal $name:literal $gid:literal $sheet:literal $knots:literal $make:expr;)*) => {
        /// Every class the factory knows.
        pub static CLASSES: &[Class] = &[
            $(Class {
                tag: Tag::new($tag),
                name: $name,
                gadget_id: $gid,
                sheet: $sheet,
                knots: $knots,
                make: $make,
            },)*
        ];
    };
}

classes! {
    b"Pend" "cPendulum" 1000 1000 1000 science::pendulum;
    b"Drop" "cDropChamber" 1001 1001 1001 science::drop_chamber;
    b"Cano" "cPhysicsCanon" 1002 1002 1002 science::physics_canon;
    b"Wtch" "cStopWatch" 1004 1004 1004 sound_time::stop_watch;
    b"rand" "cRandomNumberGenerator" 1007 1007 1007 numbers::random_number;
    b"Fork" "cTuningFork" 1008 1008 1008 sound_time::tuning_fork;
    b"DClk" "cDigitalClock" 1009 1009 1009 sound_time::digital_clock;
    b"Key_" "cKey" 1010 1010 1010 inputs::keypress;
    b"Metr" "cMetronome" 1011 1011 1011 sound_time::metronome;
    b"swit" "cSwitch" 1012 1012 1012 inputs::switch;
    b"di10" "cDisplay10" 1014 1014 1014 displays::display10;
    b"Sldr" "cSlider" 1015 1015 1015 inputs::slider;
    b"bulb" "cLightBulb" 1016 1016 1016 displays::light_bulb;
    b"hart" "cHeart" 1017 1017 1017 inputs::heart;
    b"rgbd" "cRGBDisplay" 1018 1018 1018 displays::rgb_display;
    b"Plnt" "cPlanet" 2000 1022 1022 science::planet;
    b"Ttlr" "cTotaler" 2001 1005 1005 numbers::totaler;
    b"disp" "cDisplay" 2002 1013 1013 displays::display3;
    b"ArVo" "cAreaVolume" 2003 1025 1024 science::area_volume_gadget;
    b"cnst" "cConstant" 2004 1021 1021 numbers::constant;
    b"1ncr" "cOneNumberCruncher" 2005 1024 1024 numbers::one_number_cruncher;
    b"Coin" "cCoinToss" 2006 1006 1006 numbers::coin_toss;
    b"NmKP" "cNumericKeyPad" 2007 1027 1027 inputs::numeric_keypad;
    b"AndG" "cAndGate" 2008 1029 1029 logic::and_gate;
    b"OrGa" "cOrGate" 2009 1030 1030 logic::or_gate;
    b"NotG" "cNotGate" 2010 1032 1032 logic::not_gate;
    b"XorG" "cXorGate" 2011 1031 1031 logic::xor_gate;
    b"Addr" "cAdder" 2016 1020 1020 numbers::adder;
    b"Subr" "cSubtracter" 2017 1020 1020 numbers::subtracter;
    b"Mulr" "cMultiplier" 2018 1020 1020 numbers::multiplier;
    b"Divr" "cDivider" 2019 1020 1020 numbers::divider;
    b"Conv" "cConversion" 2020 1026 1026 numbers::conversion;
    b"Comp" "cComparison" 2021 1028 1028 logic::comparison;
    b"Tape" "cTapePlayer" 2026 1037 1037 sound_time::tape_player;
    b"TxBx" "cTextBox" 2027 1047 1047 labels::text_box;
    b"TxLn" "cTextLine" 2028 1046 1046 labels::text_line;
    b"MsMv" "cMouseMove" 2029 1039 1039 inputs::mouse_move;
    b"DnPl" "cDynamitePlunger" 2030 1048 1048 inputs::dynamite_plunger;
    b"MsCl" "cMouseClick" 2032 1040 1040 inputs::mouse_click;
    b"Cntr" "cCounter" 2033 1041 1041 numbers::counter;
    b"GrPr" "cGraphicProjector" 2035 1050 1050 displays::graphic_projector;
    b"SdSw" "cSoundSwitch" 2037 1045 1045 sound_time::sound_switch;
    b"SdTr" "cSoundTransformer" 2038 1044 1044 sound_time::sound_transformer;
    b"AnSd" "cAnimalSound" 2040 1042 1042 sound_time::animal_sound;
    b"MsSd" "cMiscSound" 2041 1043 1043 sound_time::misc_sound;
    b"Spkr" "cSpeaker" 2042 1038 1038 sound_time::speaker;
    b"Stop" "cStopper" 2043 1051 1051 logic::stopper;
    b"StCh" "cStripChart" 2044 1049 1049 displays::strip_chart;
    b"TxLb" "cTextLabel" 2045 0 0 labels::text_label;
    b"MiCo" "cMiniConstant" 2046 1063 1063 numbers::mini_constant;
    b"Spl2" "cSplitter2" 2022 1019 1033 logic::splitter2;
    b"Spl3" "cSplitter3" 2023 1019 1034 logic::splitter3;
    b"Spl4" "cSplitter4" 2024 1019 1035 logic::splitter4;
    b"Mplx" "cMultiplexer" 2025 1036 1036 logic::multiplexer;
    b"Stk1" "cSticker1" 2034 1052 1052 displays::sticker;
    b"Stk2" "cSticker2" 2034 1053 1053 displays::sticker;
    b"Stk3" "cSticker3" 2034 1054 1054 displays::sticker;
    b"Stk4" "cSticker4" 2034 1055 1055 displays::sticker;
    b"Stk5" "cSticker5" 2034 1056 1056 displays::sticker;
    b"Stk6" "cSticker6" 2034 1057 1057 displays::sticker;
    b"Stk7" "cSticker7" 2034 1058 1058 displays::sticker;
    b"Stk8" "cSticker8" 2034 1059 1059 displays::sticker;
    b"Stk9" "cSticker9" 2034 1060 1060 displays::sticker;
    b"St10" "cSticker10" 2034 1061 1061 displays::sticker;
}

/// The class tagged `tag`.
#[must_use]
pub fn class(tag: Tag) -> Option<&'static Class> {
    CLASSES.iter().find(|c| c.tag == tag)
}
