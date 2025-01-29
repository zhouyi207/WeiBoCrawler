import pandas as pd
from ..util import drop_table_duplicates, Table

def process_body_resp(resp):
    """处理详细页数据

    这里一般都会收到正常的响应，所以只需要处理数据即可.
    Args:
        resp (httpx.Response): 接受到的响应.

    Returns:
        list[dict]: 响应的数据, 这里使用 list 包装一下(对齐其他的process请求).
    """
    data = resp.json()
    return [data]


def process_body_table(table: Table) -> pd.DataFrame:
    df = drop_table_duplicates(table)
    return df