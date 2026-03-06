const PDF_CONTENT_COMMANDS: [&str; 73] = [
    "w", "J", "j", "M", "d", "i", "ri", "gs", // General graphics state
    "q", "Q", "cm", // Special graphics state
    "m", "l", "c", "v", "re", "y", "h", // Path construction
    "s", "F", "f*", "B", "B*", "b", "b*", "n", "S", "f", // Path painting
    "W", "W*", // Clipping paths
    "BT", "ET", // Text objects
    "Tc", "Tw", "Tz", "TL", "Tf", "Tr", "Ts", // Text state
    "Td", "TD", "Tm", "T*", // Text positioning
    "Tj", "'", "\"", "TJ", // Text showing
    "d0", "d1", // Type 3 fonts
    "cs", "CS", "sc", "SC", "scn", "SCN", "g", "G", "rg", "RG", "k", "K",  // Color
    "sh", // Shading patterns
    "BI", "ID", "EI", // Inline images
    "Do", // XObjects
    "MP", "DP", "BMC", "BDC", "EMC", // Marked content
    "BX", "EX", // Compatibility
];

pub fn is_op(name: &[u8]) -> bool {
    for command in PDF_CONTENT_COMMANDS.iter() {
        if command.as_bytes() == name {
            return true;
        }
    }
    false
}
