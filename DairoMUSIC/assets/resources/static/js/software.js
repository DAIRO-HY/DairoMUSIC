/**
 * 依赖软件的安装卡片(DairoDFS配置、采集服务配置、依赖软件页面共用)
 * - 显示安装状态、版本号、安装日志
 * - 上传安装包或填写URL下载安装,重新安装就是更新版本
 * - 内置服务可以启动、停止、重启,查看运行日志
 * - ffmpeg等工具可以填写外部程序的路径
 */
class SoftwareCard {

    /**
     * @param selector 容器
     * @param key 软件标识 dfs、search、ffmpeg、ffprobe、yt-dlp、deno
     * @param onChange 安装状态变化时的回调(可选)
     */
    constructor(selector, key, onChange) {
        this.$box = $(selector)
        this.key = key
        this.api = "/admin/api/software/" + encodeURIComponent(key)
        this.onChange = onChange
        this.info = null
        this.uploading = false
        this.showServiceLog = false
        this.render()
        this.refresh()
    }

    render() {
        this.$box.html(`
<div class="sw-card">
    <div class="d-flex align-items-center flex-wrap gap-2">
        <span class="fw-semibold" data-name></span>
        <span data-state></span>
        <span data-service></span>
        <span class="text-secondary small" data-version></span>
    </div>
    <div class="small text-secondary mt-1 text-break">当前使用的程序: <code data-program></code></div>
    <div class="small text-danger mt-1 text-break" data-version-error></div>
    <div class="alert alert-warning small py-2 mt-2 mb-0 text-break" data-service-message style="display:none"></div>
    <div class="mt-2 d-flex flex-wrap gap-2" data-service-buttons style="display:none!important">
        <button type="button" class="btn btn-sm btn-outline-success" data-action="start"><i class="bi bi-play-fill"></i> 启动</button>
        <button type="button" class="btn btn-sm btn-outline-secondary" data-action="stop"><i class="bi bi-stop-fill"></i> 停止</button>
        <button type="button" class="btn btn-sm btn-outline-primary" data-action="restart"><i class="bi bi-arrow-repeat"></i> 重启</button>
        <button type="button" class="btn btn-sm btn-outline-secondary" data-action="service-log"><i class="bi bi-journal-text"></i> 运行日志</button>
    </div>
    <pre class="sw-log mt-2" data-service-log style="display:none"></pre>
    <div class="mt-3" data-external style="display:none">
        <label class="form-label small mb-1">外部程序路径 <span class="text-secondary">(没有安装内置程序时使用,为空时使用系统PATH里的程序)</span></label>
        <div class="input-group input-group-sm">
            <input class="form-control" data-external-path autocomplete="off">
            <button class="btn btn-outline-primary" type="button" data-action="save-path">保存</button>
        </div>
    </div>
    <div class="mt-3">
        <label class="form-label small mb-1">通过URL下载安装</label>
        <div class="input-group input-group-sm">
            <input class="form-control" data-url placeholder="安装包的下载地址(程序文件,或zip、tar.gz、tar.xz压缩包)" autocomplete="off">
            <button class="btn btn-outline-secondary" type="button" data-action="recommend">推荐地址</button>
            <button class="btn btn-outline-primary" type="button" data-action="install-url">下载安装</button>
        </div>
        <div class="small text-secondary mt-1" data-recommend></div>
    </div>
    <div class="mt-2">
        <label class="form-label small mb-1">上传安装包</label>
        <div class="input-group input-group-sm">
            <input class="form-control" type="file" data-file>
            <button class="btn btn-outline-primary" type="button" data-action="upload">上传安装</button>
        </div>
    </div>
    <div class="progress mt-2" style="height: 6px; display:none" data-progress>
        <div class="progress-bar progress-bar-striped progress-bar-animated" style="width: 0"></div>
    </div>
    <div class="small text-danger mt-2 text-break" data-install-error></div>
    <div class="mt-2 d-flex flex-wrap gap-2">
        <button type="button" class="btn btn-sm btn-link px-0" data-action="install-log">安装日志</button>
        <button type="button" class="btn btn-sm btn-link px-0" data-action="refresh">重新检查版本</button>
        <button type="button" class="btn btn-sm btn-link px-0 text-danger ms-auto" data-action="uninstall" style="display:none">卸载内置程序</button>
    </div>
    <pre class="sw-log" data-install-log style="display:none"></pre>
</div>`)
        this.$box.on("click", "[data-action]", e => this.onAction($(e.currentTarget).data("action")))
    }

    /**
     * 获取最新状态,安装中时频繁刷新
     */
    refresh() {
        clearTimeout(this.timer)
        $.getJSON(this.api + "/info").done(info => {
            const changed = this.info && (this.info.installed !== info.installed || this.info.version !== info.version
                || (this.info.service && info.service && this.info.service.status !== info.service.status))
            this.info = info
            this.show(info)
            if (changed && this.onChange) {
                this.onChange(info)
            }
        }).always(() => {
            const busy = this.uploading || (this.info && (this.info.install.running
                || (this.info.service && ["starting", "waiting"].includes(this.info.service.status))))
            this.timer = setTimeout(() => this.refresh(), busy ? 800 : 3000)
        })
        if (this.showServiceLog) {
            this.loadServiceLog()
        }
    }

    show(info) {
        const $box = this.$box
        $box.find("[data-name]").text(info.name)
        let state
        if (info.installed) {
            state = `<span class="badge text-bg-success">已安装内置程序</span>`
        } else if (info.isService) {
            state = `<span class="badge text-bg-secondary">未安装</span>`
        } else if (info.version) {
            state = `<span class="badge text-bg-info">使用外部程序</span>`
        } else {
            state = `<span class="badge text-bg-danger">不可用</span>`
        }
        $box.find("[data-state]").html(state)
        $box.find("[data-version]").text(info.version ? "版本: " + info.version : "")
        $box.find("[data-program]").text(info.program)
        $box.find("[data-version-error]").text(info.versionError ? "不能运行: " + info.versionError : "")
        $box.find("[data-action=uninstall]").toggle(info.installed)

        //外部程序路径(只有工具),正在编辑时不覆盖
        $box.find("[data-external]").toggle(!info.isService)
        const $path = $box.find("[data-external-path]")
        if (!$path.is(":focus") && !$path.data("dirty")) {
            $path.val(info.externalPath)
        }

        //内置服务的运行状态
        const service = info.service
        $box.find("[data-service-buttons]").attr("style", service && info.installed ? "" : "display:none!important")
        if (service) {
            const map = {
                stopped: ["text-bg-secondary", "已停止"],
                starting: ["text-bg-warning", "启动中"],
                running: ["text-bg-success", "运行中"],
                waiting: ["text-bg-danger", "异常退出,等待重启"],
            }
            const [cls, text] = map[service.status] || ["text-bg-secondary", service.status]
            const pid = service.pid ? ` (PID ${service.pid})` : ""
            $box.find("[data-service]").html(info.installed ? `<span class="badge ${cls}">${text}${pid}</span>` : "")
            $box.find("[data-service-message]").text(service.message).toggle(!!service.message)
        }

        //安装进度
        const install = info.install
        const running = install.running || this.uploading
        $box.find("[data-progress]").toggle(running)
        if (running) {
            const percent = install.total > 0 ? Math.min(100, install.downloaded * 100 / install.total) : 100
            $box.find(".progress-bar").css("width", percent + "%")
        }
        $box.find("[data-action=install-url], [data-action=upload]").prop("disabled", running)
        $box.find("[data-install-error]").text(install.error ? "安装失败: " + install.error : "")
        const $log = $box.find("[data-install-log]")
        $log.text(install.logs.join("\n"))
        if (running || install.error) {
            $log.show()
        }
        if ($log.is(":visible")) {
            $log.scrollTop($log[0].scrollHeight)
        }
    }

    onAction(action) {
        switch (action) {
            case "start":
            case "stop":
            case "restart":
                $.ajaxByData(this.api + "/" + action).success(() => this.refresh()).post()
                break
            case "service-log":
                this.showServiceLog = !this.showServiceLog
                this.$box.find("[data-service-log]").toggle(this.showServiceLog)
                if (this.showServiceLog) {
                    this.loadServiceLog()
                }
                break
            case "install-log":
                this.$box.find("[data-install-log]").toggle()
                break
            case "refresh":
                $.ajaxByData(this.api + "/refresh").success(() => this.refresh()).post()
                break
            case "recommend":
                this.$box.find("[data-recommend]").text("正在获取...")
                $.getJSON(this.api + "/recommend").done(data => {
                    this.$box.find("[data-url]").val(data.url)
                    this.$box.find("[data-recommend]").text(data.version ? "版本: " + data.version : "最新版本")
                }).fail(xhr => {
                    this.$box.find("[data-recommend]").text(xhr.responseText || "获取失败")
                })
                break
            case "install-url":
                const url = this.$box.find("[data-url]").val().trim()
                if (url === "") {
                    this.$box.find("[data-install-error]").text("请填写下载地址")
                    return
                }
                $.ajaxByData(this.api + "/install_url").add("url", url).success(() => this.refresh()).post()
                break
            case "upload":
                this.upload()
                break
            case "uninstall":
                if (!confirm("确定要卸载内置的" + this.info.name + "吗?")) {
                    return
                }
                $.ajaxByData(this.api + "/uninstall").success(() => this.refresh()).post()
                break
            case "save-path":
                const $path = this.$box.find("[data-external-path]")
                $.ajaxByData(this.api + "/path").add("path", $path.val()).success(() => {
                    $path.data("dirty", false)
                    this.refresh()
                }).post()
                break
        }
    }

    /**
     * 上传安装包,请求体直接是文件内容
     */
    upload() {
        const file = this.$box.find("[data-file]")[0].files[0]
        if (!file) {
            this.$box.find("[data-install-error]").text("请选择安装包")
            return
        }
        this.uploading = true
        this.$box.find("[data-install-error]").text("")
        this.$box.find("[data-install-log]").show()
        const xhr = new XMLHttpRequest()
        xhr.open("POST", this.api + "/upload?name=" + encodeURIComponent(file.name))
        xhr.upload.onprogress = e => {
            if (e.lengthComputable) {
                this.$box.find("[data-progress]").show()
                this.$box.find(".progress-bar").css("width", (e.loaded * 100 / e.total) + "%")
            }
        }
        xhr.onloadend = () => {
            this.uploading = false
            if (xhr.status === 401) {
                window.location.href = "/admin/login.html"
                return
            }
            if (xhr.status !== 200) {
                this.$box.find("[data-install-error]").text("上传失败: " + (xhr.responseText || "网络连接失败"))
            } else {
                this.$box.find("[data-file]").val("")
            }
            this.refresh()
        }
        xhr.send(file)
        this.refresh()
    }

    loadServiceLog() {
        $.getJSON(this.api + "/logs").done(lines => {
            const $log = this.$box.find("[data-service-log]")
            const atBottom = $log[0].scrollTop + $log[0].clientHeight >= $log[0].scrollHeight - 20
            $log.text(lines.join("\n"))
            if (atBottom) {
                $log.scrollTop($log[0].scrollHeight)
            }
        })
    }
}

$(function () {
    //外部程序路径修改过之后,刷新时不覆盖
    $(document).on("input", "[data-external-path]", e => $(e.currentTarget).data("dirty", true))
})
