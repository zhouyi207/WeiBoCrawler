import asyncio
from abc import ABC, abstractmethod

import httpx
from pydantic import BaseModel
from tinydb import TinyDB

from ..util import CustomProgress, request_params


class CommentID(BaseModel):
    uid: str
    mid: str


class BaseDownloader(ABC):
    def __init__(self, concurrency: int = 100):
        self.semaphore = asyncio.Semaphore(concurrency)
        self.db = None

    @abstractmethod
    def _get_request_params(self) -> list:
        pass

    @abstractmethod
    def _get_database_path(self) -> str:
        pass

    @abstractmethod
    async def _get_response_async(
        self, params, client, progress, overall_task
    ) -> httpx.Response:
        pass

    @abstractmethod
    def _get_response_sync(
        self, param, client, progress, overall_task
    ) -> httpx.Response:
        pass

    @abstractmethod
    def _process_response(self, response: httpx.Response, *, table_name: str) -> None:
        pass

    @abstractmethod
    async def _download_single_asyncio(self, param, client, progress, overall_task):
        pass

    @abstractmethod
    def _download_single_sync(self, param, client, progress, overall_task):
        pass

    def _check_response(self, response: httpx.Response) -> bool:
        return response.status_code == httpx.codes.OK

    def _save_to_database(self, items: list[dict], *, table_name: str) -> None:
        table = self.db.table(table_name)
        table.insert_multiple(items)

    async def _download_asyncio(self):
        with CustomProgress() as progress:
            overall_task = progress.add_task(
                "download...", total=len(self._get_request_params())
            )
            async with httpx.AsyncClient(cookies=request_params.cookies) as client:
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
        with CustomProgress() as progress:
            overall_task = progress.add_task(
                "download...", total=len(self._get_request_params())
            )
            with httpx.Client(cookies=request_params.cookies) as client:
                for params in self._get_request_params():
                    self._download_single_sync(params, client, progress, overall_task)

    def download(self, asynchrony: bool = True) -> None:
        self.db = TinyDB(self._get_database_path())

        if asynchrony:
            try:
                loop = asyncio.get_running_loop()
                loop.run_until_complete(self._download_asyncio())
            except RuntimeError:
                asyncio.run(self._download_asyncio())
        else:
            self._download_sync()

        self.db.close()
