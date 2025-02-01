import httpx
from datetime import datetime
from typing import Literal, Optional, Any
from ..util import CustomProgress, retry_timeout_decorator, retry_timeout_decorator_asyncio
from ..request import get_list_response_asyncio, get_list_response
from ..parse import parse_list_html
from .BaseDownloader import BaseDownloader, BodyRecord, RecordFrom


class Downloader(BaseDownloader):
    def __init__(self, search_for: str, *, table_name: str, kind : Literal["综合", "实时", "高级"] = "综合", 
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end:Optional[datetime]=None, concurrency: int = 100):
        """下载 List 页面数据, 并保存在数据库的 search_for 表中, 数据库位置在 database_config 中.

        Args:
            search_for (str): 需要搜索的内容，如果是话题，需要在 search_for 前后都加上 #
            table_name (str): 存储的位置(数据表名)
            kind (Literal[, optional): 搜索类型可以是 综合，实时，高级(添加了综合，热度，原创筛选以及时间). Defaults to "综合".
            advanced_kind (Literal[, optional): 筛选条件，可以是综合，热度，原创. Defaults to "综合".
            time_start (Optional[datetime], optional): 起始时间，最大颗粒度为小时. Defaults to Optional[datetime].
            time_end (Optional[datetime], optional): 结束时间，最大颗粒度为小时. Defaults to Optional[datetime].
            concurrency (int, optional): 异步最大并发. Defaults to 100.
        """
        super().__init__(table_name=table_name, concurrency=concurrency)

        self.search_for = search_for
        self.kind = kind
        self.advanced_kind = advanced_kind
        self.time_start = time_start
        self.time_end = time_end


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
        return list(range(1, 51))

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
                record_from=RecordFrom.Html,
                json_data = item
            )
            records.append(record)
        return records

    def _process_response(self, response: httpx.Response, *, param: Any) -> None:
        """处理请求并存储数据

        Args:
            response (httpx.Response): 需要处理的请求
            table_name (str): 存储的位置(数据表名)
        """
        items = parse_list_html(response.text)
        records = self._process_items(items)
        self._save_to_database(records)

    async def _process_response_asyncio(self, response: httpx.Response, *, param: Any) -> None:
        """处理请求并存储数据

        Args:
            response (httpx.Response): 需要处理的请求
            param (Any): 请求参数
        """
        items = parse_list_html(response.text)
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
        response = await get_list_response_asyncio(
                            search_for=self.search_for,
                            kind=self.kind,
                            advanced_kind=self.advanced_kind, 
                            time_start=self.time_start, 
                            time_end=self.time_end, 
                            page_index=param,
                            client=client)
                        
        if self._check_response(response):
            await self._process_response_asyncio(response, param=param)
        
        progress.update(overall_task, advance=1, description=f"{param}...")

    @retry_timeout_decorator
    def _download_single_sync(self, *, param: Any, client:httpx.Response, progress:CustomProgress, overall_task:int):
        """下载单个请求(同步)

        Args:
            param (Any): 请求参数
            client (httpx.Response): 请求客户端
            progress (CustomProgress): 进度条
            overall_task (int): 进度条任务ID
        """
        response = get_list_response(
                            search_for=self.search_for,
                            kind=self.kind,
                            advanced_kind=self.advanced_kind,
                            time_start=self.time_start,
                            time_end=self.time_end,
                            page_index=param,
                            client=client)
        
        if self._check_response(response):
            self._process_response(response, param=param)
        
        progress.update(overall_task, advance=1, description=f"{param}") 


def get_list_data(search_for: str, *,  table_name: str, asynchrony: bool = True, kind : Literal["综合", "实时", "高级"] = "综合", 
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end:Optional[datetime]=None) -> list:
    """获取 List 页面数据

    Args:
        search_for (str): 需要搜索的内容，如果是话题，需要在 search_for 前后都加上 #.
        table_name (str): 存储的位置(数据表名)
        asynchrony (bool, optional): _description_. Defaults to True.
        kind (Literal[, optional): 搜索类型可以是 综合，实时，高级(添加了综合，热度，原创筛选以及时间). Defaults to "综合".
        advanced_kind (Literal[, optional): 筛选条件，可以是综合，热度，原创. Defaults to "综合".
        time_start (Optional[datetime], optional): 起始时间，最大颗粒度为小时. Defaults to None.
        time_end (Optional[datetime], optional): 结束时间，最大颗粒度为小时. Defaults to None.
    
    Returns:
        list: 存储在数据库中的 id 列表
    """
    downloader = Downloader(search_for=search_for, table_name=table_name, kind=kind, advanced_kind=advanced_kind, time_start=time_start, time_end=time_end)
    downloader.download(asynchrony=asynchrony)
    return downloader.res_ids
