import struct
from PIL import Image
import numpy as np
import io

W = 400
H = 300
BITS = 8


def image_to_bit_buffer(img, output_path=None, threshold=210, out_bw_name=None):
    img_bw = img.point(lambda p: 1 if p > threshold else 0, mode="1")
    if out_bw_name is not None:
        img_bw.save(out_bw_name)

    bit_array = np.array(img_bw, dtype=np.uint8)
    bit_array.reshape(H, W)
    buffer = bytearray()
    for row in bit_array:
        for i in range(0, len(row), 8):
            byte = sum((row[i + j] << (7 - j)) for j in range(8) if i + j < len(row))
            buffer.append(byte)

    return buffer


def prepare_img(image_path):
    img = Image.open(io.BytesIO(image_path)).convert("L")
    # img.thumbnail((W, img.height))
    return img.resize((W, H))


def save_bin(buffer, image_path="uno"):
    with open(f"{image_path}.bin", "wb") as p:
        p.write(buffer)

    print(f"buff: {len(buffer)}byte")
    print("done")


if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <filename.png>")
        sys.exit(1)

    img = Image.open(image_path).convert("L")
    img.resize((W, H))
    buffer = image_to_bit_buffer(img, out_bw_name="output.png")
    save_bin(buffer, image_path=image_path)
