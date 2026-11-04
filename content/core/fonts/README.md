# HUD stroke font (`stroke_font/1`)

The HUD's text is drawn as strokes by the same pipeline as its symbols, so there is no glyph
atlas and no signed-distance-field tool (ADR-41). The font is original.

**Grid.** Each glyph is polylines on a 4 x 6 grid (`cell`), `y` up from the baseline, advancing 6
units per character. A space has no strokes. Points may be on half units.

**Coverage.** The baker requires `0-9`, `A-Z`, space and `. - + / % : ° α`: what the HUD's numbers
and texts use. The warning texts need `α` (`α-LIM`); the Mach number needs `.`; the vertical-speed
and load-factor numbers need `-` and `+`. The renderer's test checks every character of every HUD
string and number format against the font, so adding a string with a new character fails there
until the glyph exists. Lower case is drawn as capitals.

The glyphs are plain and legible rather than elegant; the zero is slashed so it is not the letter
O.
