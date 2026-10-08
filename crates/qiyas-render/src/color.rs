use opencadcodec::Color;

const WHITE: [u8; 3] = [255, 255, 255];

/// The colour to draw with. ByLayer takes the layer's colour; ByBlock and None draw white;
/// an index goes through the ACI table (index 7 is white there); true colour passes through.
/// The frontend swaps white and black to suit the theme.
pub fn resolve_color(entity_color: &Color, layer_color: &Color) -> [u8; 3] {
    let color = match entity_color {
        Color::ByLayer => layer_color,
        other => other,
    };
    color.rgb().map_or(WHITE, |(r, g, b)| [r, g, b])
}
