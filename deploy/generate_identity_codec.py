"""Regenerate the identity compatibility table from the pinned Python reference."""
from pathlib import Path
import unicodedata

def generate():
    if unicodedata.unidata_version!='15.0.0':raise RuntimeError('Use Python 3.12 / Unicode 15.0.0; changing identity normalization is a migration')
    rows=[]
    for number in range(0x110000):
        if 0xD800<=number<=0xDFFF:continue
        char=chr(number);folded=char.casefold()
        if folded!=char:
            value=''.join('\\u{'+format(ord(c),'x')+'}' for c in folded)
            rows.append("    ('\\u{"+format(number,'x')+"}', \""+value+'\"),')
    return '\n'.join(['//! Generated identity compatibility data. Unicode 15.0.0 / Python 3.12.',
        '//! Regenerate with deploy/generate_identity_codec.py. Unicode data license applies.',
        'pub const FOLDS: &[(char, &str)] = &[',*rows,'];',''])
if __name__=='__main__':
    root=Path(__file__).resolve().parents[1]
    (root/'rust/crates/core/src/casefold.rs').write_text(generate())
