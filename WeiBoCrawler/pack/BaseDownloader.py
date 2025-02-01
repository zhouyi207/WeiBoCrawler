import asyncio
from abc import ABC, abstractmethod
from typing import Any

import httpx
from pydantic import BaseModel
from ..database import db, BodyRecord, Comment1Record, Comment2Record, RecordFrom
from ..util import CustomProgress, cookies_config, log_function_params, logging


logger = logging.getLogger(__name__)

class CommentID(BaseModel):
    uid: str
    mid: str


class BaseDownloader(ABC):
    def __init__(self, *, table_name: str, concurrency: int = 100):
        self.table_name = table_name
        self.semaphore = asyncio.Semaphore(concurrency)
        self.db = db
        self.res_ids = []

    @abstractmethod
    def _get_request_description(self) -> str:
        """获取进度条描述

        Returns:
            str: 进度条描述
        """
        ...

    @abstractmethod
    def _get_request_params(self) -> list:
        """获取请求参数列表

        Returns:
            list: 请求参数列表
        """
        ...

    @abstractmethod
    def _process_response(self, response: httpx.Response, *, param: Any) -> None:
        """处理请求并存储数据

        Args:
            response (httpx.Response): 需要处理的请求
            param (Any): 请求参数
        """
        ...

    @abstractmethod
    async def _process_response_asyncio(self, response: httpx.Response, *, param: Any) -> None:
        """处理请求并存储数据

        Args:
            response (httpx.Response): 需要处理的请求
            param (Any): 请求参数
        """
        ...

    @abstractmethod
    async def _download_single_asyncio(self, *, param:Any, client:httpx.Response, progress:CustomProgress, overall_task:int):
        """下载单个请求(异步)

        Args:
            param (Any): 请求参数
            client (httpx.Response): 请求客户端
            progress (CustomProgress): 进度条
            overall_task (int): 进度条任务ID
        """
        ...

    @abstractmethod
    def _download_single_sync(self, *, param: Any, client:httpx.Response, progress:CustomProgress, overall_task:int):
        """下载单个请求(同步)

        Args:
            param (Any): 请求参数
            client (httpx.Response): 请求客户端
            progress (CustomProgress): 进度条
            overall_task (int): 进度条任务ID
        """
        ...

    def _save_to_database(self, items: list[BodyRecord | Comment1Record | Comment2Record]) -> None:
        """保存数据到数据库

        Args:
            items (list[dict]): 数据列表
        """
        res_ids = self.db.sync_add_records(items)
        self.res_ids.extend(res_ids)

    async def _save_to_database_asyncio(self, items: list[BodyRecord | Comment1Record | Comment2Record]) -> None:
        """保存数据到数据库(异步)

        Args:
            items (list[dict]): 数据列表
        """
        res_ids = await self.db.async_add_records(items)
        self.res_ids.extend(res_ids)

    @log_function_params(logger=logger)
    def _check_response(self, response: httpx.Response) -> bool:
        """检查响应是否正常

        Args:
            response (httpx.Response): 接受到的响应

        Returns:
            bool: 有问题返回 False, 否则返回 True
        """
        return response.status_code == httpx.codes.OK


    async def _download_asyncio(self):
        """异步下载数据

        """
        with CustomProgress() as progress:
            overall_task = progress.add_task(
                description=self._get_request_description(), total=len(self._get_request_params())
            )
            async with httpx.AsyncClient(cookies=cookies_config.cookies) as client:
                tasks = []
                for param in self._get_request_params():
                    async with self.semaphore:
                        task = asyncio.create_task(
                            self._download_single_asyncio(
                                param=param,
                                client=client,
                                progress=progress,
                                overall_task=overall_task,
                            )
                        )
                        tasks.append(task)
                await asyncio.gather(*tasks)

    def _download_sync(self):
        """同步下载数据

        """
        with CustomProgress() as progress:
            overall_task = progress.add_task(
                description=self._get_request_description(), total=len(self._get_request_params())
            )
            with httpx.Client(cookies=cookies_config.cookies) as client:
                for params in self._get_request_params():
                    self._download_single_sync(params, client, progress, overall_task)

    def download(self, asynchrony: bool = True) -> None:
        """整合异步下载和同步下载

        asynchrony = True 异步下载
        asynchrony = False 普通下载

        Args:
            asynchrony (bool, optional): 异步下载或者普通下载. Defaults to True.
        """
        if asynchrony:
            try:
                loop = asyncio.get_running_loop()
                loop.run_until_complete(self._download_asyncio())
            except RuntimeError:
                asyncio.run(self._download_asyncio())
        else:
            self._download_sync()


__all__ = [BaseDownloader, BodyRecord, Comment1Record, Comment2Record, RecordFrom]