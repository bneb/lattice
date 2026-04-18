import numpy as np
from PIL import Image

try:
    img = Image.open('tests/output/e2e_diff_mask.png')
    arr = np.array(img)
    if len(arr.shape) == 3:
        arr = np.mean(arr, axis=2)
    
    # Scale down by factor of 30 for ascii
    h, w = arr.shape
    new_h, new_w = h // 30, w // 30
    
    ascii_arr = np.zeros((new_h, new_w), dtype=np.uint8)
    for i in range(new_h):
        for j in range(new_w):
            chunk = arr[i*30:(i+1)*30, j*30:(j+1)*30]
            ascii_arr[i, j] = 1 if np.mean(chunk) > 10 else 0
            
    for i in range(new_h):
        line = ""
        for j in range(new_w):
            line += "#" if ascii_arr[i, j] else "."
        print(line)

except Exception as e:
    print(e)
