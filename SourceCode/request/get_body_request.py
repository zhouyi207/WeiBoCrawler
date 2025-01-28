import httpx
from .util import requestparams


def get_body_response(id: str) -> httpx.Response:
    """获取微博详细页的请求结果
    微博详细页id位置(https://weibo.com/{userid}/{id}?{params}):
        1. 找到需要爬取的微博内容页, 比如:
            https://weibo.com/1644114654/OiZre8dir?refer_flag=1001030103_  -> id = OiZre8dir

    Args:
        id (str): 微博详细页id.

    Returns:
        httpx.Response: 返回的请求结果.
    """
    headers = requestparams.body_headers
    cookies = requestparams.cookies
    url = "https://weibo.com/ajax/statuses/show"
    params = {
        "id": f"{id}",
        "locale": "zh-CN",
        "isGetLongText": "true"
    }
    response = httpx.get(url, headers=headers, cookies=cookies, params=params)
    return response


async def get_body_response_asyncio(id:str, *, client: httpx.AsyncClient) -> httpx.Response:
    """获取微博详细页的请求结果(异步)
    微博详细页id位置(https://weibo.com/{userid}/{id}?{params}):
        1. 找到需要爬取的微博内容页, 比如:
            https://weibo.com/1644114654/OiZre8dir?refer_flag=1001030103_  -> id = OiZre8dir

    Args:
        id (str): 微博详细页id.
        client (httpx.AsyncClient): 异步客户端.

    Returns:
        httpx.Response: 返回的请求结果.
    """
    headers = requestparams.body_headers
    cookies = requestparams.cookies
    url = "https://weibo.com/ajax/statuses/show"
    params = {
        "id": f"{id}",
        "locale": "zh-CN",
        "isGetLongText": "true"
    }
    response = await client.get(url, headers=headers, cookies=cookies, params=params)
    return response