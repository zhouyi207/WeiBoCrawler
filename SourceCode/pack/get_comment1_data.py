import httpx
from ..request.get_comment_request import get_comments_l1_response, get_comments_l1_response_asyncio
from ..parse.process_comment import process_comment_resp
from typing import List, Union, Any
from ..util import CustomProgress
from ..util import database_config
from .BaseDownloader import BaseDownloader, CommentID


class Downloader(BaseDownloader):
    def __init__(self, *, uid: Union[List[str], str], mid: Union[List[str], str], concurrency: int = 100, max_failed_times: int = 20) -> None:
        """根据 uid 和 mid 下载评论数据，并保存在数据库的 mid 表中, 数据库位置在 database_config 中

        Args:
            uid (Union[List[str], str]): 用户 ID
            mid (Union[List[str], str]): 信息 ID
            concurrency (int, optional): 最大异步并发. Defaults to 100.
            max_failed_times (int, optional): 最大失败次数. Defaults to 20.

        Raises:
            ValueError: uid and mid must be both str or list and the length of uid and mid must be equal.
        """
        super().__init__(concurrency=concurrency)

        if isinstance(uid, str) and isinstance(mid, str):
            self.ids = [CommentID(uid=uid, mid=mid)]
        elif isinstance(uid, list) and isinstance(mid, list) and len(uid) == len(mid):
            self.ids = [CommentID(uid=u, mid=m) for u, m in zip(uid, mid)]
        else:
            raise ValueError("uid and mid must be both str or list and the length of uid and mid must be equal")
        
        self.max_failed_times = max_failed_times


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

    def _get_database_path(self) -> str:
        """获取数据库路径

        Returns:
            str: 数据库路径
        """
        return database_config.comment1

    def _process_response(self, response: httpx.Response, *, table_name: str) -> None:
        """处理请求并存储数据

        Args:
            response (httpx.Response): 需要处理的请求
            table_name (str): 存储的位置(数据表名)
        """
        resp_info, items = process_comment_resp(response)
        self._save_to_database(items, table_name=table_name)
        return resp_info

    async def _download_single_asyncio(self, *, param:Any, client:httpx.Response, progress:CustomProgress, overall_task:int):
        """下载单个请求(异步)
        1. 在这里首先处理第一个评论，因为第一个评论是不需要 max_id 的，所以这里单独处理
        2. 处理每一个评论响应的时候，通过 _process_response 方法获取到 resp_info
        3. 其中 resp_info 包含 max_id, total_number, data_number. 其中 max_id 用于下一个请求, total_number 和 data_number 用于判断是否下载完成
        4. comment 请求有其独有的特性, 在请求次数较多时, 会出现请求失败的情况, 一般来说 failed_times 的上限为 15, 这里取保守值 20.

        Args:
            param (Any): 请求参数
            client (httpx.Response): 请求客户端
            progress (CustomProgress): 进度条
            overall_task (int): 进度条任务ID
        """
        response = await get_comments_l1_response_asyncio(uid=param.uid, mid=param.mid, client=client)
        if self._check_response(response):
            resp_info = self._process_response(response, table_name=param.mid)
            max_id = resp_info.max_id
            total_number = resp_info.total_number
            count_data_number = resp_info.data_number
            failed_times = 0 if resp_info.data_number != 0 else 1

            task = progress.add_task(completed=count_data_number, total=total_number, description=f"{param.mid}: failed_times - {failed_times}")
                        
            while (failed_times < self.max_failed_times) and (count_data_number < total_number):
                response = await get_comments_l1_response_asyncio(uid=param.uid, mid=param.mid, client=client, max_id=max_id)
                if self._check_response(response):
                    resp_info = self._process_response(response, table_name=param.mid)
                    max_id = resp_info.max_id
                    count_data_number += resp_info.data_number
                    failed_times = 0 if resp_info.data_number != 0 else failed_times + 1

                    progress.update(task, completed=count_data_number, total=total_number, description=f"{param.mid}: failed_times - {failed_times}")

                else:
                    failed_times += 1

            progress.remove_task(task)
        progress.update(overall_task, advance=1, description=f"{param.mid}")

    def _download_single_sync(self, *, param: Any, client:httpx.Response, progress:CustomProgress, overall_task:int):
        """下载单个请求(同步)
        1. 在这里首先处理第一个评论，因为第一个评论是不需要 max_id 的，所以这里单独处理
        2. 处理每一个评论响应的时候，通过 _process_response 方法获取到 resp_info
        3. 其中 resp_info 包含 max_id, total_number, data_number. 其中 max_id 用于下一个请求, total_number 和 data_number 用于判断是否下载完成
        4. comment 请求有其独有的特性, 在请求次数较多时, 会出现请求失败的情况, 一般来说 failed_times 的上限为 15, 这里取保守值 20.

        Args:
            param (Any): 请求参数
            client (httpx.Response): 请求客户端
            progress (CustomProgress): 进度条
            overall_task (int): 进度条任务ID
        """
        response = get_comments_l1_response(uid=param.uid, mid=param.mid, client=client)
        if self._check_response(response):
            resp_info = self._process_response(response, table_name=param.mid)
            max_id = resp_info.max_id
            total_number = resp_info.total_number
            count_data_number = resp_info.data_number
            failed_times = 0 if resp_info.data_number != 0 else 1

            task = progress.add_task(completed=count_data_number, total=total_number, description=f"{param.mid}: failed_times - {failed_times}")
                        
            while (failed_times < self.max_failed_times) and (count_data_number < total_number):
                response = get_comments_l1_response(uid=param.uid, mid=param.mid, client=client, max_id=max_id)
                if self._check_response(response):
                    resp_info = self._process_response(response, table_name=param.mid)
                    max_id = resp_info.max_id
                    count_data_number += resp_info.data_number
                    failed_times = 0 if resp_info.data_number != 0 else failed_times + 1

                    progress.update(task, completed=count_data_number, total=total_number, description=f"{param.mid}: failed_times - {failed_times}")

                else:
                    failed_times += 1

            progress.remove_task(task)
        progress.update(overall_task, advance=1, description=f"{param.mid}")


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