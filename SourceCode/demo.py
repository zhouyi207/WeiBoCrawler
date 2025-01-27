import sys
import httpx
from tinydb import TinyDB

sys.path.append("./SourceCode")

from request.util import requestparams
from request.get_comment_request import get_omments_l1_response

size = []


max_id = "140943109076852"
db = TinyDB('db.json')

client = httpx.Client(cookies=requestparams.cookies)
get_omments_l1_response("2803301701", "5127059131334865", True, None, client=client)
for i in range(100): 
    resp = get_omments_l1_response("2803301701", "5127059131334865", False, max_id, client=client)
    max_id = resp.json()["max_id"]
    size.append(len(resp.text))
    print(len(resp.text))
    print(len(resp.content))
    data = resp.json()["data"]
    db.insert_multiple(data)
    print(i, "*"*50)

print(size)