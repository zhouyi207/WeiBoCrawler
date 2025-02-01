import httpx
from .util import request_headers
from PIL import Image
from io import BytesIO
import time



def get_login_signin_response(client:httpx.Client) -> httpx.Response:
    """主要是获取 cookies 中的 X-CSRF-TOKEN 字段

    Args:
        client (httpx.Client): 会话客户端

    Returns:
        httpx.Response: 目的是获取响应的 url
    """
    headers = request_headers.login_signin_headers

    url = "https://passport.weibo.com/sso/signin"
    params = {
        "entry": "miniblog",
        "source": "miniblog",
        "disp": "popup",
        "url": "https://weibo.com/newlogin?tabtype=weibo&gid=102803&openLoginLayer=0&url=https%3A%2F%2Fweibo.com%2F",
        "from": "weibopro"
    }
    
    response = client.get(url, params=params, headers=headers)
    response.raise_for_status()
    return response


def get_login_qrcode_response(client:httpx.Client, login_signin_url:str) -> httpx.Response:
    """主要是获取二维码的 id 以及 二维码的 url 路径

    Args:
        client (httpx.Client): 会话客户端
        login_signin_url (str): signin 请求的url 主要是需要设置 referer 字段

    Returns:
        httpx.Response: 主要是获取 qrid 字段 和 二维码的 url
    """
    headers = request_headers.login_qrcode_headers
    headers["referer"] = login_signin_url
    headers["x-csrf-token"] = client.cookies.get("X-CSRF-TOKEN")

    url = "https://passport.weibo.com/sso/v2/qrcode/image"
    params = {
        "entry": "miniblog",
        "size": "180"
    }
    response = client.get(url, params=params, headers=headers)
    response.raise_for_status()
    return response
    

def get_login_check_response(client:httpx.Client, login_signin_url:str, qrid:str) -> httpx.Response:
    """检查二维码状态：未使用，已扫描未确认，已确认，已过期

    Args:
        client (httpx.Client): 会话客户端
        login_signin_url (str): signin 请求的url 主要是需要设置 referer 字段
        qrid (str): 二维码的 id

    Returns:
        httpx.Response: 检查二维码状态
    """
    headers = request_headers.login_final_headers
    headers["referer"] = login_signin_url
    headers["x-csrf-token"] = client.cookies["X-CSRF-TOKEN"]

    url = "https://passport.weibo.com/sso/v2/qrcode/check"
    params = {
        "entry": "miniblog",
        "source": "miniblog",
        "url": "https://weibo.com/newlogin?tabtype=weibo&gid=102803&openLoginLayer=0&url=https%3A%2F%2Fweibo.com%2F",
        "qrid": qrid,
        "disp": "popup"
    }
    response = client.get(url, headers=headers, params=params)
    response.raise_for_status()
    return response



def get_login_final_response(client:httpx.Client, login_url:str) -> httpx.Response:
    """最终的登录请求

    Args:
        client (httpx.Client): 会话客户端
        login_url (str): 最终的登入 url

    1. 在这里由于是重定向请求，所有在 client 中最好设置 follow_redirects=True.
    2. 最终的 response 不知道为啥一直是 403 请求，但是 cookies 是成功获取得到了的.
    
    Returns:
        httpx.Response: 没啥用
    """
    response = client.get(login_url)
    # response.raise_for_status()
    return response


def download_and_open_image(url:str):
    """下载并打开图片用来扫描

    Args:
        url (str): 二维码图片地址
    """
    try:
        response = httpx.get(url)
        response.raise_for_status()
        image_content = BytesIO(response.content)
        image = Image.open(image_content)
        image.show()
    except httpx.RequestError as e:
        print(f"请求发生错误: {e}")
    except Exception as e:
        print(f"发生其他错误: {e}")
    


def get_cookies():
    """最终获取 cookies 的函数

    Returns:
        dict: 获取的 cookies
    """
    client = httpx.Client(follow_redirects=True)

    login_signin_response = get_login_signin_response(client)
    login_signin_url = str(login_signin_response.url)

    login_qrcode_response = get_login_qrcode_response(client, login_signin_url=login_signin_url)
    qrcode_json_data = login_qrcode_response.json().get("data")

    qrid = qrcode_json_data.get("qrid")
    image_path = qrcode_json_data.get("image")
    download_and_open_image(image_path)


    count = 0
    while count <= 25:
        login_check_response = get_login_check_response(client, login_signin_url=login_signin_url, qrid=qrid)
        login_check_response.encoding = "utf-8"
        login_check_json_data = login_check_response.json()

        if login_check_json_data.get("retcode") == 20000000:
            login_url = login_check_json_data.get("data").get("url")
            break

        print(f"二维码状态码: {login_check_json_data.get('retcode')}, 状态信息: {login_check_json_data.get('msg')}")
        time.sleep(1)
        count += 1
    else:
        return None

    # 这里的 response 是一个重定向的响应, 其最终结果状态是 403 但是好像在重定向的过程中会设置一些 cookie 信息
    get_login_final_response(client, login_url=login_url)
    
    return dict(client.cookies)