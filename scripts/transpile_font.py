import sys
import os
from PIL import Image, ImageDraw, ImageFont

def ensure_font(filename):
    paths = [
        f"/System/Library/Fonts/{filename}",
        f"/Library/Fonts/{filename}",
        filename
    ]
    for p in paths:
        if os.path.exists(p):
            return p
    return None

def render_font_array(font_path, array_name, faux_bold=False):
    if not font_path:
        print(f"Warning: font path empty for {array_name}, generating blank fallback.")
        return f"pub global {array_name}: [u8; 2048] = [0; 2048];\n"
        
    try:
        # 14pt SF Mono fits nicely in an 8x16 box natively on MacOS.
        font = ImageFont.truetype(font_path, 14)
        # Using y_offset to perfectly center standard characters.
        y_offset = -2
        x_offset = 0
    except IOError:
        print(f"Error: Could not load {font_path}")
        return f"pub global {array_name}: [u8; 2048] = [0; 2048];\n"
        
    out = f"pub global {array_name}: [u8; 2048] = [\n"
    
    for c in range(128):
        img = Image.new("L", (8, 16), 0)
        draw = ImageDraw.Draw(img)
        
        char = chr(c)
        if c < 32 or c == 127:
            char = "" 
            
        draw.text((x_offset, y_offset), char, font=font, fill=255)
        
        bytes_out = []
        for y in range(16):
            byte_val = 0
            for x in range(8):
                pixel = img.getpixel((x, y))
                if pixel > 120:
                    byte_val |= (1 << (7 - x))
                    # Faux Bold logic: inflate rightwards
                    if faux_bold and x < 7:
                        byte_val |= (1 << (7 - (x + 1)))
            bytes_out.append(str(byte_val))
            
        out += "    " + ", ".join(bytes_out) + ","
        if c == 32:
            out += " // [SPACE]"
        elif c > 32 and c < 127:
            out += f" // '{chr(c)}'"
        out += "\n"
        
    for c in range(128, 256):
        out += "    " + ", ".join(["0"] * 16) + ",\n"
        
    out += "];\n"
    return out

def main():
    reg_font = ensure_font("SFNSMono.ttf")
    it_font = ensure_font("SFNSMonoItalic.ttf")
    
    with open("user/terminal/font_data.salt", "w") as f:
        f.write("package user.terminal.font_data\n\n")
        f.write(render_font_array(reg_font, "FONT_REGULAR_8X16", False))
        f.write(render_font_array(reg_font, "FONT_BOLD_8X16", True))
        f.write(render_font_array(it_font, "FONT_ITALIC_8X16", False))

if __name__ == '__main__':
    main()
