import httpx
import asyncio
from tinydb import TinyDB
from datetime import datetime
from ..type import CustomProgress
from typing import Literal, Optional
from ..database.util import database_config
from ..request.get_list_request import get_list_response_asyncio, get_list_response
from ..parse.parse_list_html import parse_list_html



class Downloader:
    def __init__(self, search_for: str, *,  kind : Literal["综合", "实时", "高级"] = "综合", 
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end:Optional[datetime]=None, concurrency: int = 100):
        self.semaphore = asyncio.Semaphore(concurrency)
        self.search_for = search_for
        self.kind = kind
        self.advanced_kind = advanced_kind
        self.time_start = time_start
        self.time_end = time_end
        
        self.db = ""
        self.table = ""


    async def _download_asyncio(self):
        with CustomProgress() as progress:
            task = progress.add_task("download...", total=50)
            async with self.semaphore:
                async with httpx.AsyncClient() as client:
                    for i in range(50):
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

                        progress.update(task, advance=1, description=f"{i}")

    def _download(self):
        with CustomProgress() as progress:
            task = progress.add_task("download...", total=50)
            for i in range(50):
                resp = get_list_response(search_for=self.search_for, page_index=i+1, kind=self.kind, advanced_kind=self.advanced_kind, time_start=self.time_start, time_end=self.time_end)

                if self._check_response(resp):
                    self._process_response(resp)
                progress.update(task, advance=1, description=f"{i+1:2d}/50")


    def _check_response(self, response:httpx.Response) -> bool:
        return response.status_code == httpx.codes.OK


    def _process_response(self, response:httpx.Response) -> None:
        items = parse_list_html(response.text)
        self._save_to_database(items)


    def _save_to_database(self, items:list) -> None:
        self.table.insert_multiple(items)


    def download(self, asynchrony:bool = True) -> None:
        """下载数据
        
        asynchrony = True 异步下载, 平均时间为 20s
        asynchrony = False 普通下载, 平均时间为 30s
        
        - 差距好像也不是很大（0.0）
        
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
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end:Optional[datetime]=None):

    downloader = Downloader(search_for=search_for, kind=kind, advanced_kind=advanced_kind, time_start=time_start, time_end=time_end)
    downloader.download(asynchrony=asynchrony)
