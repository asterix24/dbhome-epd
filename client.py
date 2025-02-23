import socket
import struct
import time
from PIL import Image
import numpy as np

UDP_IP = "192.168.1.115"
UDP_PORT = 23000
TIMEOUT = 5
W = 400
H = 300
BITS = 8

def image_to_bit_buffer(image_path, output_path=None):
    img = Image.open(image_path).convert("L")
    img = img.resize((400, 300))

    threshold = 128
    img_bw = img.point(lambda p: 1 if p > threshold else 0, mode="1")
    img_bw.save("out.png")

    bit_array = np.array(img_bw, dtype=np.uint8).reshape(300, 400)
    buffer = bytearray()
    for row in bit_array:
        for i in range(0, len(row), 8):
            byte = sum((row[i + j] << (7 - j)) for j in range(8) if i + j < len(row))
            buffer.append(byte)

    return buffer


def send(buffer):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    chunk = min(1024 - 8, len(buffer))
    frame_len = len(buffer)
    offset = 0
    while frame_len > 0:
        hdr = struct.pack("iI", offset, chunk)
        sock.sendto((hdr + buffer[offset:offset+chunk]), (UDP_IP, UDP_PORT))

        print(f"{offset:6}, {frame_len:6}, {chunk:6}")

        frame_len -= chunk
        offset += chunk

        chunk = min(1024 - 8, frame_len)
        time.sleep(0.1)

    print(f"{offset:6}, {frame_len:6}, {chunk:6}")
    hdr = struct.pack("iI", -1, 0)
    print(f"Frame ends: {hdr}")
    sock.sendto(hdr, (UDP_IP, UDP_PORT))


if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <filename.png>")
        sys.exit(1)

    buffer = image_to_bit_buffer(sys.argv[1], "output.bin")
    print(f"buff{len(buffer)}byte")
    send(buffer)
    print("done")
