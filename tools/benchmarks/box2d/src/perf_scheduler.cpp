#include "perf_scheduler.h"

#include <algorithm>
#include <condition_variable>
#include <cstdint>
#include <deque>
#include <memory>
#include <mutex>
#include <thread>
#include <vector>

struct TaskHandle {
    b2TaskCallback* callback;
    void* context;
    std::mutex mutex;
    std::condition_variable finished;
    int remaining;
};

struct WorkItem {
    TaskHandle* handle;
    int start;
    int end;
};

struct physbox_scheduler {
    explicit physbox_scheduler(int count) : stopping(false), step_tasks(0) {
        workers.reserve(static_cast<std::size_t>(count));
        for (int index = 0; index < count; ++index) {
            workers.emplace_back([this, index] { worker_loop(index); });
        }
    }

    ~physbox_scheduler() {
        {
            std::lock_guard<std::mutex> lock(queue_mutex);
            stopping = true;
        }
        ready.notify_all();
        for (std::thread& worker : workers) {
            worker.join();
        }
    }

    void worker_loop(std::uint32_t worker_index) {
        while (true) {
            WorkItem item{};
            {
                std::unique_lock<std::mutex> lock(queue_mutex);
                ready.wait(lock, [this] { return stopping || !queue.empty(); });
                if (stopping && queue.empty()) {
                    return;
                }
                item = queue.front();
                queue.pop_front();
            }
            item.handle->callback(item.start, item.end, worker_index,
                                  item.handle->context);
            {
                std::lock_guard<std::mutex> lock(item.handle->mutex);
                --item.handle->remaining;
                if (item.handle->remaining == 0) {
                    item.handle->finished.notify_all();
                }
            }
        }
    }

    std::mutex queue_mutex;
    std::condition_variable ready;
    std::deque<WorkItem> queue;
    std::vector<std::thread> workers;
    bool stopping;
    int step_tasks;
};

extern "C" physbox_scheduler* physbox_scheduler_create(int worker_count) {
    if (worker_count < 2) {
        return nullptr;
    }
    return new physbox_scheduler(worker_count);
}

extern "C" void physbox_scheduler_destroy(physbox_scheduler* scheduler) {
    delete scheduler;
}

extern "C" void physbox_scheduler_begin_step(physbox_scheduler* scheduler) {
    if (scheduler != nullptr) {
        scheduler->step_tasks = 0;
    }
}

extern "C" void* physbox_enqueue_task(b2TaskCallback* task, int item_count,
                                        int min_range, void* task_context,
                                        void* user_context) {
    auto* scheduler = static_cast<physbox_scheduler*>(user_context);
    if (scheduler == nullptr || item_count <= 0) {
        task(0, item_count, 0, task_context);
        return nullptr;
    }
    const int workers = static_cast<int>(scheduler->workers.size());
    const int range = std::max(min_range, 1);
    const int job_count = std::max(1, std::min(workers, (item_count + range - 1) / range));
    auto* handle = new TaskHandle{task, task_context, {}, {}, job_count};
    {
        std::lock_guard<std::mutex> lock(scheduler->queue_mutex);
        for (int job = 0; job < job_count; ++job) {
            const int start = item_count * job / job_count;
            const int end = item_count * (job + 1) / job_count;
            scheduler->queue.push_back(WorkItem{handle, start, end});
        }
        ++scheduler->step_tasks;
    }
    scheduler->ready.notify_all();
    return handle;
}

extern "C" void physbox_finish_task(void* user_task, void*) {
    auto* handle = static_cast<TaskHandle*>(user_task);
    if (handle == nullptr) {
        return;
    }
    {
        std::unique_lock<std::mutex> lock(handle->mutex);
        handle->finished.wait(lock, [handle] { return handle->remaining == 0; });
    }
    delete handle;
}
