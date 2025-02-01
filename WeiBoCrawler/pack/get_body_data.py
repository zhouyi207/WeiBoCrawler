import httpx
from typing import Any
from ..util import CustomProgress, retry_timeout_decorator, retry_timeout_decorator_asyncio
from ..parse import process_body_resp
from .BaseDownloader import BaseDownloader, BodyRecord, RecordFrom
from ..request import get_body_response, get_body_response_asyncio


class Downloader(BaseDownloader):
    def __init__(self, id: list[str] | str, *, table_name: str, concurrency: int = 100):
        """下载 Body 页面数据, 并保存在数据库的 id 表中, 数据库位置在 database_config 中.

        Args:
            id (Union[List[str], str]): 微博详细页 id, 或者 id 列表.
            table_name (str): 存储的位置(数据表名)
            concurrency (int, optional): 异步最大并发. Defaults to 100.
        """
        super().__init__(table_name=table_name, concurrency=concurrency)

        if isinstance(id, str):
            self.ids = [id]
        else:
            self.ids = id

    def _get_request_description(self) -> str:
        """获取进度条描述

        Returns:
            str: 进度条描述
        """
        return "download..."

    def _get_request_params(self) -> list:
        """获取请求参数列表

        Returns:
            list: 请求参数列表
        """
        return self.ids

    def _process_items(self, items: list[dict]) -> list[BodyRecord]:
        """_summary_

        Args:
            items (list[dict]): _description_

        Returns:
            list[BodyRecord]: _description_
        """
        records = []
        for item in items:
            mid = item.get("mid", None)
            uid = item.get("uid", None)
            record = BodyRecord(
                mid=mid,
                uid=uid,
                search_for=self.table_name,
                record_from=RecordFrom.Api,
                json_data = item
            )
            records.append(record)
        return records

    def _process_response(self, response: httpx.Response, *, param: Any) -> None:
        """处理请求并存储数据

        Args:
            response (httpx.Response): 需要处理的请求
            param (Any): 请求参数
        """
        items = process_body_resp(response)
        records = self._process_items(items)
        self._save_to_database(records)

    async def _process_response_asyncio(self, response: httpx.Response, *, param: Any) -> None:
        """处理请求并存储数据

        Args:
            response (httpx.Response): 需要处理的请求
            param (Any): 请求参数
        """
        items = process_body_resp(response)
        records = self._process_items(items)
        await self._save_to_database_asyncio(records)

    @retry_timeout_decorator_asyncio
    async def _download_single_asyncio(self, *, param:Any, client:httpx.Response, progress:CustomProgress, overall_task:int):
        """下载单个请求(异步)

        Args:
            param (Any): 请求参数
            client (httpx.Response): 请求客户端
            progress (CustomProgress): 进度条
            overall_task (int): 进度条任务ID
        """
        response = await get_body_response_asyncio(
                            id=param,
                            client=client)
                        
        if self._check_response(response):
            await self._process_response_asyncio(response, param=param)
        
        progress.update(overall_task, advance=1, description=f"{param}")

    @retry_timeout_decorator
    def _download_single_sync(self, *, param: Any, client:httpx.Response, progress:CustomProgress, overall_task:int):
        """下载单个请求(同步)

        Args:
            param (Any): 请求参数
            client (httpx.Response): 请求客户端
            progress (CustomProgress): 进度条
            overall_task (int): 进度条任务ID
        """
        response = get_body_response(
                            id=param,
                            client=client)
        if self._check_response(response):
            self._process_response(response, param=param)
        
        progress.update(overall_task, advance=1, description=f"{param}") 



def get_body_data(id: list[str] | str, *, table_name:str, asynchrony: bool = True) -> list:
    """获取 body 页面数据

    Args:
        id (Union[List[str], str]): 微博详细页 id, 或者 id 列表.
        table_name (str): 存储的位置(数据表名)
        asynchrony (bool, optional): _description_. Defaults to True.

    Returns:
        list: 存储在数据库中的 id 列表
    """
    downloader = Downloader(id = id, table_name=table_name)
    downloader.download(asynchrony=asynchrony)
    return downloader.res_ids
