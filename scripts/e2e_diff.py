import numpy as np
from PIL import Image
import sys

def mse(imageA, imageB):
    err = np.sum((imageA.astype("float") - imageB.astype("float")) ** 2)
    err /= float(imageA.shape[0] * imageA.shape[1])
    return err

def main():
    try:
        baseline = Image.open('tests/fixtures/hn_chrome_truth.png').convert('RGB')
        test = Image.open('tests/output/prisimi_hn_render.png').convert('RGB')
    except Exception as e:
        print(f"[FAIL] Missing files for diff: {e}")
        sys.exit(1)
        
    baseline_np = np.array(baseline)
    test_np = np.array(test)
    
    h = min(baseline_np.shape[0], test_np.shape[0])
    w = min(baseline_np.shape[1], test_np.shape[1])
    
    baseline_crop = baseline_np[:h, :w, :]
    test_crop = test_np[:h, :w, :]
    
    error = mse(baseline_crop, test_crop)
    
    print(f"[E2E] Structural Visual Diff Check")
    print(f"MSE: {error:.4f} (Threshold bounds checking)")
    
    # Generate visual diff mask
    diff_mask = np.abs(baseline_crop.astype("float") - test_crop.astype("float")).astype('uint8')
    diff_img = Image.fromarray(diff_mask)
    diff_img.save('tests/output/e2e_diff_mask.png')
    print("[E2E] Wrote structural visual diff to tests/output/e2e_diff_mask.png")
    
    # Allowing for reasonable font rendering differences (Chrome skia vs Metal Truetype)
    if error > 25000:
        print("[FAIL] Structural rendering layout strongly deviates from Chromium baseline!")
        sys.exit(1)
    else:
        print("[OK] Structural rendering within matching tolerances.")
        sys.exit(0)

if __name__ == "__main__":
    main()
