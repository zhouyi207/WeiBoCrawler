import httpx
from .util import requestparams


def get_body_response(id):
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