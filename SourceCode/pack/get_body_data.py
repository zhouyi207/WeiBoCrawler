from ..database.util import database_config
from ..request.get_body_request import get_body_response, get_body_response_asyncio
from typing import List, Union
import asyncio
from ..type import CustomProgress
import httpx
from ..parse.process_body_json import process_body_resp
from tinydb import TinyDB


class Downloader:
    def __init__(self, id: Union[List[str], str], *, concurrency: int = 100):
        """下载 Body 页面数据, 并保存在数据库的 id 表中, 数据库位置在 database_config 中.

        Args:
            id (Union[List[str], str]): 微博详细页 id, 或者 id 列表.
            concurrency (int, optional): 异步最大并发. Defaults to 100.
        """

        self.semaphore = asyncio.Semaphore(concurrency)
        
        if isinstance(id, str):
            self.ids = [id]
        else:
            self.ids = id
        
        self.db = ""
        self.table = ""


    async def _download_asyncio(self):
        """异步下载数据

        """
        with CustomProgress() as progress:
            task = progress.add_task("download...", total=len(self.ids))
            async with self.semaphore:
                async with httpx.AsyncClient() as client:
                    for id in self.ids:
                        resp = await get_body_response_asyncio(
                            id=id,
                            client=client)
                        
                        if self._check_response(resp):
                            self._process_response(resp, id=id)

                        progress.update(task, advance=1, description=f"")


    def _download(self):
        """正常下载数据

        """
        with CustomProgress() as progress:
            task = progress.add_task("download...", total=len(self.ids))
            for id in self.ids:
                resp = get_body_response(id=id)

                if self._check_response(resp):
                    self._process_response(resp, id=id)

                progress.update(task, advance=1, description=f"")


    def _check_response(self, response:httpx.Response) -> bool:
        """检查响应是否正常

        Args:
            response (httpx.Response): 接受到的响应

        Returns:
            bool: 有问题返回 False, 否则返回 True
        """
        return response.status_code == httpx.codes.OK


    def _process_response(self, response:httpx.Response, *, id:str) -> None:
        """处理响应
        1. 首先解析响应
        2. 然后保存到数据库中

        Args:
            response (httpx.Response): 接受到的响应
            id (str): 微博详细页 id
        """
        item = process_body_resp(response)
        self._save_to_database(item, id=id)


    def _save_to_database(self, item:dict, *, id:str) -> None:
        """将 dict 数据保存到数据库中

        Args:
            item (dict): 接受到的 dict 数据
            id (str): 微博详细页 id
        """
        self.table = self.db.table(id)
        self.table.insert(item)


    def download(self, asynchrony:bool = True) -> None:
        """下载数据
        
        asynchrony = True 异步下载, 平均 id 耗时为 42/213 = 0.19s
        asynchrony = False 普通下载, 平均 id 耗时为 88/213 = 0.41s
        
        - 差距为已被，但是好像也不是很大（0.0）
        
        Args:
            asynchrony (bool, optional): 异步下载或者普通下载. Defaults to True.
        """
        self.db = TinyDB(database_config.body)
        

        if asynchrony:
            try:
                loop = asyncio.get_running_loop()
                loop.run_until_complete(self._download_asyncio())
            except RuntimeError:
                asyncio.run(self._download_asyncio())
        else:
            self._download()

        self.db.close()



def get_body_data(id: Union[List[str], str], *,  asynchrony: bool = True) -> None:
    """获取 body 页面数据

    Args:
        id (Union[List[str], str]): 微博详细页 id, 或者 id 列表.
        asynchrony (bool, optional): _description_. Defaults to True.
    """
    downloader = Downloader(id = id)
    downloader.download(asynchrony=asynchrony)
