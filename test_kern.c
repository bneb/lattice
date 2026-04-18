#define STB_TRUETYPE_IMPLEMENTATION
#include "vendor/stb/stb_truetype.h"
#include "vendor/stb/font_data.h"
#include <stdio.h>

int main() {
    stbtt_fontinfo font;
    if (!stbtt_InitFont(&font, vendor_stb_Roboto_Regular_ttf, 0)) return 1;
    
    for (int i = 32; i < 127; i++) {
        for (int j = 32; j < 127; j++) {
            int advance = stbtt_GetCodepointKernAdvance(&font, i, j);
            if (advance != 0) {
                printf("%c%c: %d\n", i, j, advance);
            }
        }
    }
    return 0;
}
