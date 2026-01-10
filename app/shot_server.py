import socket
import struct
import time


def send(sock, buffer):
    chunk = min(1024 - 8, len(buffer))
    frame_len = len(buffer)
    offset = 0
    while frame_len > 0:
        hdr = struct.pack("iI", offset, chunk)
        data = hdr + buffer[offset : offset + chunk]
        sock.sendall(data)

        print(f"server: {offset:6}, {frame_len:6}, {chunk:6}, {len(data):6}")

        frame_len -= chunk
        offset += chunk

        chunk = min(1024 - 8, frame_len)
        time.sleep(0.5)

    print(f"{offset:6}, {frame_len:6}, {chunk:6}")
    hdr = struct.pack("iI", -1, 0)
    sock.sendall(hdr)
    print(f"Frame ends: {hdr}")


def run_binary_server(buffer, host="0.0.0.0", port=22000):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind((host, port))
        s.listen()

        print(f"Server listen on {host}:{port} file: {len(buffer)}")
        conn, addr = s.accept()
        with conn:
            print(f"Server conn from: {addr}")
            send(conn, buffer)
            print(f"Server sent: {len(buffer)} bytes")


if __name__ == "__main__":
    import sys, os

    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <filename.png>")
        sys.exit(1)

    bin_file = sys.argv[1]
    while True:
        if os.path.isfile(bin_file):
            buffer = open(bin_file, "rb").read()
            run_binary_server(buffer)
            os.remove(bin_file)
        time.sleep(30)
