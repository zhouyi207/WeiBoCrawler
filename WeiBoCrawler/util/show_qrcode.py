from pyzbar.pyzbar import decode
from PIL import Image
import qrcode

def show_qrcode(img_path:str):
    """在控制台显示二维码

    Args:
        img_path (str): 二维码路径
    """
    img = Image.open('gen.png')
    decoded_data = decode(img)
    data = decoded_data[0].data.decode('utf-8')
    qr = qrcode.QRCode()
    qr.add_data(data)
    qr.make()
    qr.print_ascii()