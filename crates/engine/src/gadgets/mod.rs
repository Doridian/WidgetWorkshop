//! The gadget classes: each one's tag, numbers and behaviour. The factory
//! (`0x41f194`) creates a class from its tag; the numbers come from each
//! class's init function (the sheet it loads with `0x422940`, the knot
//! table with `0x41db6c`, the gadget id it stores at `+0x28`).

use crate::gadget::{Behavior, Tag, Unknown};

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

fn unknown() -> Box<dyn Behavior> {
    Box::new(Unknown::default())
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
    b"Pend" "cPendulum" 1000 1000 1000 unknown;
    b"Drop" "cDropChamber" 1001 1001 1001 unknown;
    b"Cano" "cPhysicsCanon" 1002 1002 1002 unknown;
    b"Wtch" "cStopWatch" 1004 1004 1004 unknown;
    b"rand" "cRandomNumberGenerator" 1007 1007 1007 unknown;
    b"Fork" "cTuningFork" 1008 1008 1008 unknown;
    b"DClk" "cDigitalClock" 1009 1009 1009 unknown;
    b"Key_" "cKey" 1010 1010 1010 unknown;
    b"Metr" "cMetronome" 1011 1011 1011 unknown;
    b"swit" "cSwitch" 1012 1012 1012 unknown;
    b"di10" "cDisplay10" 1014 1014 1014 unknown;
    b"Sldr" "cSlider" 1015 1015 1015 unknown;
    b"bulb" "cLightBulb" 1016 1016 1016 unknown;
    b"hart" "cHeart" 1017 1017 1017 unknown;
    b"rgbd" "cRGBDisplay" 1018 1018 1018 unknown;
    b"Plnt" "cPlanet" 2000 1022 1022 unknown;
    b"Ttlr" "cTotaler" 2001 1005 1005 unknown;
    b"disp" "cDisplay" 2002 1013 1013 unknown;
    b"ArVo" "cAreaVolume" 2003 1025 1024 unknown;
    b"cnst" "cConstant" 2004 1021 1021 unknown;
    b"1ncr" "cOneNumberCruncher" 2005 1024 1024 unknown;
    b"Coin" "cCoinToss" 2006 1006 1006 unknown;
    b"NmKP" "cNumericKeyPad" 2007 1027 1027 unknown;
    b"AndG" "cAndGate" 2008 1029 1029 unknown;
    b"OrGa" "cOrGate" 2009 1030 1030 unknown;
    b"NotG" "cNotGate" 2010 1032 1032 unknown;
    b"XorG" "cXorGate" 2011 1031 1031 unknown;
    b"Addr" "cAdder" 2016 1020 1020 unknown;
    b"Subr" "cSubtracter" 2017 1020 1020 unknown;
    b"Mulr" "cMultiplier" 2018 1020 1020 unknown;
    b"Divr" "cDivider" 2019 1020 1020 unknown;
    b"Conv" "cConversion" 2020 1026 1026 unknown;
    b"Comp" "cComparison" 2021 1028 1028 unknown;
    b"Tape" "cTapePlayer" 2026 1037 1037 unknown;
    b"TxBx" "cTextBox" 2027 1047 1047 unknown;
    b"TxLn" "cTextLine" 2028 1046 1046 unknown;
    b"MsMv" "cMouseMove" 2029 1039 1039 unknown;
    b"DnPl" "cDynamitePlunger" 2030 1048 1048 unknown;
    b"MsCl" "cMouseClick" 2032 1040 1040 unknown;
    b"Cntr" "cCounter" 2033 1041 1041 unknown;
    b"GrPr" "cGraphicProjector" 2035 1050 1050 unknown;
    b"SdSw" "cSoundSwitch" 2037 1045 1045 unknown;
    b"SdTr" "cSoundTransformer" 2038 1044 1044 unknown;
    b"AnSd" "cAnimalSound" 2040 1042 1042 unknown;
    b"MsSd" "cMiscSound" 2041 1043 1043 unknown;
    b"Spkr" "cSpeaker" 2042 1038 1038 unknown;
    b"Stop" "cStopper" 2043 1051 1051 unknown;
    b"StCh" "cStripChart" 2044 1049 1049 unknown;
    b"TxLb" "cTextLabel" 2045 0 0 unknown;
    b"MiCo" "cMiniConstant" 2046 1063 1063 unknown;
    b"Spl2" "cSplitter2" 2022 1019 1033 unknown;
    b"Spl3" "cSplitter3" 2023 1019 1034 unknown;
    b"Spl4" "cSplitter4" 2024 1019 1035 unknown;
    b"Mplx" "cMultiplexer" 2025 1036 1036 unknown;
    b"Stk1" "cSticker1" 2034 1052 1052 unknown;
    b"Stk2" "cSticker2" 2034 1053 1053 unknown;
    b"Stk3" "cSticker3" 2034 1054 1054 unknown;
    b"Stk4" "cSticker4" 2034 1055 1055 unknown;
    b"Stk5" "cSticker5" 2034 1056 1056 unknown;
    b"Stk6" "cSticker6" 2034 1057 1057 unknown;
    b"Stk7" "cSticker7" 2034 1058 1058 unknown;
    b"Stk8" "cSticker8" 2034 1059 1059 unknown;
    b"Stk9" "cSticker9" 2034 1060 1060 unknown;
    b"St10" "cSticker10" 2034 1061 1061 unknown;
}

/// The class tagged `tag`.
#[must_use]
pub fn class(tag: Tag) -> Option<&'static Class> {
    CLASSES.iter().find(|c| c.tag == tag)
}
