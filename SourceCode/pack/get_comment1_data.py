import httpx
import asyncio
from ..request.get_comment_request import get_comments_l1_response, get_comments_l1_response_asyncio
from ..parse.process_comment import process_comment_resp
from pydantic import BaseModel
from typing import List, Union
from ..util import CustomProgress
from tinydb import TinyDB
from ..util import database_config
from ..util import request_params


class CommentID(BaseModel):
    uid: str
    mid: str


class Downloader:
    def __init__(self, *, uid: Union[List[str], str], mid: Union[List[str], str], concurrency: int = 100) -> None:
        """根据 uid 和 mid 下载评论数据，并保存在数据库的 mid 表中, 数据库位置在 database_config 中

        Args:
            uid (Union[List[str], str]): 用户 ID
            mid (Union[List[str], str]): 信息 ID
            concurrency (int, optional): 最大异步并发. Defaults to 100.

        Raises:
            ValueError: uid and mid must be both str or list and the length of uid and mid must be equal.
        """
        self.semaphore = asyncio.Semaphore(concurrency)

        if isinstance(uid, str) and isinstance(mid, str):
            self.ids = [CommentID(uid=uid, mid=mid)]
        elif isinstance(uid, list) and isinstance(mid, list) and len(uid) == len(mid):
            self.ids = [CommentID(uid=u, mid=m) for u, m in zip(uid, mid)]
        else:
            raise ValueError("uid and mid must be both str or list and the length of uid and mid must be equal")

        self.db = ''

    def _check_response(self, response: httpx.Response) -> bool:
        """检查响应是否正常

        Args:
            response (httpx.Response): 接受到的响应

        Returns:
            bool: 有问题返回 False, 否则返回 True
        """
        return response.status_code == httpx.codes.OK

    def _save_to_database(self, items: List[dict], *, id: CommentID) -> None:
        """将 请求得到的数据 List[dict] 保存到数据库中

        Args:
            items (List[dict]): 请求得到的数据.
            id (CommentID): 主要使用 CommentID 的 mid 作为表名.
        """
        table = self.db.table(id.mid)
        table.insert_multiple(items)

    def _process_response(self, response: httpx.Response, *, id: CommentID) -> dict:
        """处理响应
        1. 首先解析响应
        2. 然后保存到数据库中

        Args:
            response (httpx.Response): 接受到的响应.
            id (CommentID): 主要使用 CommentID 的 mid 作为数据库请求中的参数.
        """
        resp_info, items = process_comment_resp(response)
        self._save_to_database(items, id=id)
        return resp_info

    def _download_single_normal(self, *, id: CommentID, client: httpx.Client, max_failed_times: int = 20, progress: CustomProgress, overall_task: int):
        """处理单个下载
        1. 在这里首先处理第一个评论，因为第一个评论是不需要 max_id 的，所以这里单独处理
        2. 处理每一个评论响应的时候，通过 _process_response 方法获取到 resp_info
        3. 其中 resp_info 包含 max_id, total_number, data_number. 其中 max_id 用于下一个请求, total_number 和 data_number 用于判断是否下载完成
        4. comment 请求有其独有的特性, 在请求次数较多时, 会出现请求失败的情况, 一般来说 failed_times 的上限为 15, 这里取保守值 20.
        Args:
            id (CommentID): CommentID 包装起来的 uid 和 mid.
            client (httpx.Client): 接受到的 client.
            max_failed_times (int): 最大失败次数. Defaults to 20.
            progress (CustomProgress): 进度条.
            overall_task: 整体进度条任务.
        """

        task = progress.add_task("download...")

        resp = get_comments_l1_response(uid=id.uid, mid=id.mid, client=client)
        if self._check_response(resp):
            resp_info = self._process_response(resp, id=id)
            max_id = resp_info.max_id
            total_number = resp_info.total_number
            count_data_number = resp_info.data_number
            failed_times = 0 if resp_info.data_number!= 0 else 1

            progress.update(task, completed=count_data_number, total=total_number, description=f"{id.mid}: failed_times - {failed_times}")
            
            while (failed_times < max_failed_times) and (count_data_number < total_number):
                resp = get_comments_l1_response(uid=id.uid, mid=id.mid, client=client, max_id=max_id)
                if self._check_response(resp):
                    resp_info = self._process_response(resp, id=id)
                    max_id = resp_info.max_id
                    count_data_number += resp_info.data_number
                    failed_times = 0 if resp_info.data_number!= 0 else failed_times + 1

                    progress.update(task, completed=count_data_number, total=total_number, description=f"{id.mid}: failed_times - {failed_times}")
                
                else:
                    failed_times += 1

        progress.remove_task(task)
        progress.update(overall_task, advance=1, description=f"总")

    def _download_normal(self):
        """正常下载数据

        """
        with CustomProgress() as progress:
            overall_task = progress.add_task("download...", total=len(self.ids))
            with httpx.Client(cookies=request_params.cookies) as client:
                for id in self.ids:
                    self._download_single_normal(
                        id=id,
                        progress=progress,
                        client=client,
                        overall_task=overall_task)

    async def _download_single_asyncio(self, *, id: CommentID, client: httpx.AsyncClient, max_failed_times: int = 20, progress: CustomProgress, overall_task:int):
        """处理单个下载
        1. 在这里首先处理第一个评论，因为第一个评论是不需要 max_id 的，所以这里单独处理
        2. 处理每一个评论响应的时候，通过 _process_response 方法获取到 resp_info
        3. 其中 resp_info 包含 max_id, total_number, data_number. 其中 max_id 用于下一个请求, total_number 和 data_number 用于判断是否下载完成
        4. comment 请求有其独有的特性, 在请求次数较多时, 会出现请求失败的情况, 一般来说 failed_times 的上限为 15, 这里取保守值 20.

        Args:
            id (CommentID): CommentID 包装起来的 uid 和 mid.
            client (httpx.AsyncClient): 接受到的 client.
            max_failed_times (int): 最大失败次数. Defaults to 20.
            progress (CustomProgress): 进度条.
            overall_task: 整体进度条任务
        """
        task = progress.add_task("download...")

        resp = await get_comments_l1_response_asyncio(uid=id.uid, mid=id.mid, client=client)
        if self._check_response(resp):
            resp_info = self._process_response(resp, id=id)
            max_id = resp_info.max_id
            total_number = resp_info.total_number
            count_data_number = resp_info.data_number
            failed_times = 0 if resp_info.data_number != 0 else 1

            progress.update(task, completed=count_data_number, total=total_number, description=f"{id.mid}: failed_times - {failed_times}")

            while (failed_times < max_failed_times) and (count_data_number < total_number):
                resp = await get_comments_l1_response_asyncio(uid=id.uid, mid=id.mid, client=client, max_id=max_id)
                if self._check_response(resp):
                    resp_info = self._process_response(resp, id=id)
                    max_id = resp_info.max_id
                    count_data_number += resp_info.data_number
                    failed_times = 0 if resp_info.data_number != 0 else failed_times + 1

                    progress.update(task, completed=count_data_number, total=total_number, description=f"{id.mid}: failed_times - {failed_times}")
                
                else:
                    failed_times += 1
            
        progress.remove_task(task)
        progress.update(overall_task, advance=1, description=f"总")

    async def _download_asyncio(self):
        """异步下载数据

        """
        with CustomProgress() as progress:
            overall_task = progress.add_task("download...", total=len(self.ids))
            async with httpx.AsyncClient(cookies=request_params.cookies) as client:
                tasks = []
                for id in self.ids:
                    async with self.semaphore:
                        task = asyncio.create_task(self._download_single_asyncio(
                            id=id,
                            progress=progress,
                            client=client,
                            overall_task=overall_task))
                        tasks.append(task)

                await asyncio.gather(*tasks)

    def download(self, asynchrony: bool = True) -> None:
        self.db = TinyDB(database_config.comment1)

        if asynchrony:
            try:
                loop = asyncio.get_running_loop()
                loop.run_until_complete(self._download_asyncio())
            except RuntimeError:
                asyncio.run(self._download_asyncio())
        else:
            self._download_normal()

        self.db.close()


def get_comment1_data(uid: Union[List[str], str], mid: Union[List[str], str], *, asynchrony: bool = True) -> None:
    """根据 uid 和 mid 下载评论数据，并保存在数据库的 mid 表中, 数据库位置在 database_config 中

    Args:
        uid (Union[List[str], str]): 用户 ID
        mid (Union[List[str], str]): 信息 ID
        concurrency (int, optional): 最大异步并发. Defaults to 100.

    Raises:
        ValueError: uid and mid must be both str or list and the length of uid and mid must be equal.
    """
    downloader = Downloader(uid=uid, mid=mid)
    downloader.download(asynchrony=asynchrony)