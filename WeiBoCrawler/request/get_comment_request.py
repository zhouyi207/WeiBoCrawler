import httpx
from .util import request_headers
from typing import Optional

def build_comments_l1_params(uid: str, mid : str, *, max_id: Optional[str]=None) -> tuple:
    """构建微博主体一级评论的参数

    Args:
        uid (str): 微博的uid
        mid (str): 微博的mid
        max_id (str, optional): 是否是第一次请求该微博主体的评论, 如果是第一次, max_id 设置为 None; 否则设置为 max_id. Defaults to None.

    Returns:
        tuple: (url, params, headers).
    """
    url = "https://weibo.com/ajax/statuses/buildComments"
    headers = request_headers.comment1_buildComments_headers

    params = {
        "is_reload": "1",
        "id": f"{mid}",
        "is_show_bulletin": "2",
        "is_mix": "0",
        "count": "20",
        "uid": f"{uid}",
        "fetch_level": "0",
        "locale": "zh-CN",
    }
    if max_id is not None:
        params["flow"] = "0"
        params["max_id"] = max_id

    return url, params, headers


def build_comments_l2_params(uid: str, mid : str, *, max_id: Optional[str]=None) -> tuple:
    """构建微博主体二级评论的参数

    Args:
        uid (str): 微博的uid
        mid (str): 微博的mid
        max_id (str, optional): 是否是第一次请求该微博主体的评论, 如果是第一次, max_id 设置为 None; 否则设置为 max_id. Defaults to None.

    Returns:
        tuple: (url, params, headers).
    """
    url = "https://weibo.com/ajax/statuses/buildComments"
    headers = request_headers.comment2_buildComments_headers
    
    params = {
        "flow": "0", # 0 表示按热度, 1 表示按时间
        "is_reload": "1",
        "id": f"{mid}",
        "is_show_bulletin": "2",
        "is_mix": "1",
        "fetch_level": "1",
        "count": "20",
        "uid": f"{uid}",
        "locale": "zh-CN"
    }

    if max_id is not None:
        params["max_id"] = max_id
    else:
        params["max_id"] = "0"

    return url, params, headers



def get_comments_l1_response(uid: str, mid : str, *, client: httpx.Client, max_id: Optional[str]=None) -> httpx.Response:
    """获取微博主体的一级评论

    Args:
        uid (str): 微博的uid
        mid (str): 微博的mid
        client (httpx.Client): 需要的client
        max_id (str, optional): 是否是第一次请求该微博主体的评论, 如果是第一次, max_id 设置为 None; 否则设置为 max_id. Defaults to None.

    Returns:
        httpx.Response: 评论的响应
    """
    url, params, headers = build_comments_l1_params(uid, mid, max_id=max_id)       
    response = client.get(url, params=params, headers=headers)
    return response


async def get_comments_l1_response_asyncio(uid: str, mid : str, *, client: httpx.AsyncClient, max_id: Optional[str]=None) -> httpx.Response:
    """获取微博主体的一级评论(异步)

    Args:
        uid (str): 微博的uid
        mid (str): 微博的mid
        client (httpx.AsyncClient): 需要的client
        max_id (str, optional): 是否是第一次请求该微博主体的评论, 如果是第一次, max_id 设置为 None; 否则设置为 max_id. Defaults to None.

    Returns:
        httpx.Response: 评论的响应
    """
    url, params, headers = build_comments_l1_params(uid, mid, max_id=max_id)   
    response = await client.get(url, params=params, headers=headers)
    return response


def get_comments_l2_response(uid: str, mid : str, *, client: httpx.Client, max_id: Optional[str]=None):
    """获取微博主体的二级评论

    Args:
        uid (str): 微博的uid
        mid (str): 微博的mid
        client (httpx.Client): 需要的client
        max_id (str, optional): 是否是第一次请求该微博主体的评论, 如果是第一次, max_id 设置为 None; 否则设置为 max_id. Defaults to None.

    Returns:
        httpx.Response: 评论的响应
    """
    url, params, headers = build_comments_l2_params(uid, mid, max_id=max_id)
    response = client.get(url, params=params, headers=headers)
    return response


async def get_comments_l2_response_asyncio(uid: str, mid : str, *, client: httpx.AsyncClient, max_id: Optional[str]=None):
    """获取微博主体的二级评论(异步)

    Args:
        uid (str): 微博的uid
        mid (str): 微博的mid
        client (httpx.AsyncClient): 需要的client
        max_id (str, optional): 是否是第一次请求该微博主体的评论, 如果是第一次, max_id 设置为 None; 否则设置为 max_id. Defaults to None.

    Returns:
        httpx.Response: 评论的响应
    """
    url, params, headers = build_comments_l2_params(uid, mid, max_id=max_id)
    response = await client.get(url, params=params, headers=headers)
    return response