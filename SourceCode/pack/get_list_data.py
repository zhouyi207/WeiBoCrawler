import httpx
import asyncio
from tinydb import TinyDB
from datetime import datetime
from ..util import CustomProgress
from typing import Literal, Optional
from ..database.util import database_config
from ..request.get_list_request import get_list_response_asyncio, get_list_response
from ..parse.parse_list_html import parse_list_html



class Downloader:
    def __init__(self, search_for: str, *,  kind : Literal["综合", "实时", "高级"] = "综合", 
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end:Optional[datetime]=None, concurrency: int = 100):
        """下载 List 页面数据, 并保存在数据库的 search_for 表中, 数据库位置在 database_config 中.

        Args:
            search_for (str): 需要搜索的内容，如果是话题，需要在 search_for 前后都加上 #
            kind (Literal[, optional): 搜索类型可以是 综合，实时，高级(添加了综合，热度，原创筛选以及时间). Defaults to "综合".
            advanced_kind (Literal[, optional): 筛选条件，可以是综合，热度，原创. Defaults to "综合".
            time_start (Optional[datetime], optional): 起始时间，最大颗粒度为小时. Defaults to Optional[datetime].
            time_end (Optional[datetime], optional): 结束时间，最大颗粒度为小时. Defaults to Optional[datetime].
            concurrency (int, optional): 异步最大并发. Defaults to 100.
        """
        self.semaphore = asyncio.Semaphore(concurrency)
        self.search_for = search_for
        self.kind = kind
        self.advanced_kind = advanced_kind
        self.time_start = time_start
        self.time_end = time_end
        
        self.db = ""
        self.table = ""

    async def _download_asyncio_task(self, i, client, progress, overall_task) -> None:
        resp = await get_list_response_asyncio(
                            search_for=self.search_for,
                            page_index=i+1,
                            kind=self.kind, 
                            advanced_kind=self.advanced_kind, 
                            time_start=self.time_start, 
                            time_end=self.time_end, 
                            client=client)
                        
        if self._check_response(resp):
            self._process_response(resp)

        progress.update(overall_task, advance=1, description=f"{i}")

    async def _download_asyncio(self):
        """异步下载数据

        """
        with CustomProgress() as progress:
            overall_task = progress.add_task("download...", total=50)
            async with httpx.AsyncClient() as client:
                tasks = []
                for i in range(50):
                    async with self.semaphore:
                        task = asyncio.create_task(self._download_asyncio_task(
                            i=i,
                            client=client,
                            progress=progress,
                            overall_task=overall_task))

                        tasks.append(task)
                await asyncio.gather(*tasks)
                        

    def _download(self):
        """正常下载数据

        """
        with CustomProgress() as progress:
            task = progress.add_task("download...", total=50)
            for i in range(50):
                resp = get_list_response(search_for=self.search_for, page_index=i+1, kind=self.kind, advanced_kind=self.advanced_kind, time_start=self.time_start, time_end=self.time_end)

                if self._check_response(resp):
                    self._process_response(resp)
                progress.update(task, advance=1, description=f"{i+1:2d}/50")


    def _check_response(self, response:httpx.Response) -> bool:
        """检查响应是否正常

        Args:
            response (httpx.Response): 接受到的响应

        Returns:
            bool: 有问题返回 False, 否则返回 True
        """
        return response.status_code == httpx.codes.OK


    def _process_response(self, response:httpx.Response) -> None:
        """处理响应
        1. 首先解析网页
        2. 然后保存到数据库中

        Args:
            response (httpx.Response): 接受到的响应
        """
        items = parse_list_html(response.text)
        self._save_to_database(items)


    def _save_to_database(self, items:list) -> None:
        """将 list[dict] 数据保存到数据库中

        Args:
            items (list): 接受到的 list[dict] 数据
        """
        self.table.insert_multiple(items)


    def download(self, asynchrony:bool = True) -> None:
        """下载数据
        
        asynchrony = True 异步下载, 平均时间为 <1s/50
        asynchrony = False 普通下载, 平均时间为 30s/50
        
        
        Args:
            asynchrony (bool, optional): 异步下载或者普通下载. Defaults to True.
        """
        self.db = TinyDB(database_config.list)
        self.table = self.db.table(self.search_for)

        if asynchrony:
            try:
                loop = asyncio.get_running_loop()
                loop.run_until_complete(self._download_asyncio())
            except RuntimeError:
                asyncio.run(self._download_asyncio())
        else:
            self._download()

        self.db.close()


def get_list_data(search_for: str, *,  asynchrony: bool = True, kind : Literal["综合", "实时", "高级"] = "综合", 
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end:Optional[datetime]=None) -> None:
    """获取 List 页面数据

    Args:
        search_for (str): 需要搜索的内容，如果是话题，需要在 search_for 前后都加上 #.
        asynchrony (bool, optional): _description_. Defaults to True.
        kind (Literal[, optional): 搜索类型可以是 综合，实时，高级(添加了综合，热度，原创筛选以及时间). Defaults to "综合".
        advanced_kind (Literal[, optional): 筛选条件，可以是综合，热度，原创. Defaults to "综合".
        time_start (Optional[datetime], optional): 起始时间，最大颗粒度为小时. Defaults to None.
        time_end (Optional[datetime], optional): 结束时间，最大颗粒度为小时. Defaults to None.
    """
    downloader = Downloader(search_for=search_for, kind=kind, advanced_kind=advanced_kind, time_start=time_start, time_end=time_end)
    downloader.download(asynchrony=asynchrony)
