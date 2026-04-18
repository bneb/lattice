import numpy as np
from PIL import Image

try:
    img = Image.open('/Users/kevin/.gemini/antigravity/brain/a3535a57-cc6c-472f-a667-84719dcad70f/e2e_diff_mask.png')
    arr = np.array(img)
    # Sum across color channels
    if len(arr.shape) == 3:
        arr = np.sum(arr, axis=2)
    
    # Find bounding box of errors
    rows = np.any(arr > 50, axis=1)
    cols = np.any(arr > 50, axis=0)
    
    if np.any(rows):
        rmin, rmax = np.where(rows)[0][[0, -1]]
        cmin, cmax = np.where(cols)[0][[0, -1]]
        print(f"Error Bounding Box: Y:{rmin}-{rmax}, X:{cmin}-{cmax}")
        print(f"Max Y in image: {arr.shape[0]}")
    else:
        print("No massive errors found?!")

except Exception as e:
    print(e)
