# rum 不需要构建

import httpx
import json

def get_rum_level_one_response(buildComments_url):
    
    headers = {
        "accept": "*/*",
        "accept-language": "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6",
        "content-type": "multipart/form-data; boundary=----WebKitFormBoundaryvnSjtxxxjv6x1pFT",
        "origin": "https://weibo.com",
        "priority": "u=1, i",
        "referer": "https://weibo.com/2803301701/PblVL5Bg5",
        "sec-ch-ua": "\"Not A(Brand\";v=\"8\", \"Chromium\";v=\"132\", \"Microsoft Edge\";v=\"132\"",
        "sec-ch-ua-mobile": "?0",
        "sec-ch-ua-platform": "\"Windows\"",
        "sec-fetch-dest": "empty",
        "sec-fetch-mode": "cors",
        "sec-fetch-site": "same-origin",
        "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0",
        "x-xsrf-token": "seBDSEeh70cZTEWGWWkFmxxG"
    }

    cookies = {
        "SCF": "AnQhEA08TUG9ln2r7R0-cHMvj3KTSZb-85kfIcXTHqooYhjTcn-UkaGS5792LpSqqbJApBlXrIheowZ1k4aYR1Q.",
        "SUB": "_2A25Kkj8dDeRhGeFJ4lIT9CzNyj6IHXVp7j7VrDV8PUNbmtAYLVT5kW9NfsmQ4UzJuUOhUQbYBkUvv3HADVVzl9Ig",
        "SUBP": "0033WrSXqPxfM725Ws9jqgMF55529P9D9W5Oj.LmOvr7_7fS8d6lYxiZ5JpX5KzhUgL.FoMN1K5EShzpeKz2dJLoIp7LxKML1KBLBKnLxKqL1hnLBoMNS0.7eoBEeK2E",
        "ALF": "02_1740495949",
        "SINAGLOBAL": "970667482772.5692.1737903974414",
        "ULV": "1737903974460:1:1:1:970667482772.5692.1737903974414:",
        "XSRF-TOKEN": "seBDSEeh70cZTEWGWWkFmxxG",
        "WBPSESS": "2bPq4LTfaY-EnTnt8h5hWX9KGoz50scMNqd4lpDCT8IiCLnpv2C9Z_Kk8JVbYkIyBQ0eFNYccRFpnV_A6ntYbwjqG_PAbMAldrAdPPf_XvQiQHrkm_9GFJunwjaIeUwiupJQv3fNpU5K1Xq-CCdaFg=="
    }

    entry = {
                "name": "https://weibo.com/ajax/statuses/buildComments?flow=0&is_reload=1&id=5127059131334865&is_show_bulletin=2&is_mix=0&max_id=139293859600042&count=20&uid=2803301701&fetch_level=0&locale=zh-CN",
                "entryType": "resource",
                "startTime": "327212.7000000002",
                "duration": "493.20000000018626",
                "initiatorType": "xmlhttprequest",
                "deliveryType": "",
                "nextHopProtocol": "h2",
                "renderBlockingStatus": "non-blocking",
                "workerStart": 0,
                "redirectStart": 0,
                "redirectEnd": 0,
                "fetchStart": "327212.7000000002",
                "domainLookupStart": "327212.7000000002",
                "domainLookupEnd": "327212.7000000002",
                "connectStart": "327212.7000000002",
                "secureConnectionStart": "327212.7000000002",
                "connectEnd": "327212.7000000002",
                "requestStart": "327226.7000000002",
                "responseStart": "327702.6000000001",
                "firstInterimResponseStart": 0,
                "responseEnd": "327705.9000000004",
                "transferSize": 11971,
                "encodedBodySize": 11671,
                "decodedBodySize": 72237,
                "responseStatus": 200,
                "serverTiming": [],
                "dns": 0,
                "tcp": 0,
                "ttfb": "475.89999999990687",
                "pathname": "https://weibo.com/ajax/statuses/buildComments",
                "speed": 0
            }

    files = {
            "entry": (None, json.dumps(entry)),
            "request_id": (None, ""),
        }

    url = "https://weibo.com/ajax/log/rum"
    response = httpx.post(url, headers=headers, cookies=cookies, files=files)

    print(response.headers)

url = "https://weibo.com/ajax/statuses/buildComments?flow=0&is_reload=1&id=139293862124853&is_show_bulletin=2&is_mix=0&max_id=139568722411765&count=20&uid=2803301701&fetch_level=0&locale=zh-CN"
get_rum_level_one_response(url)