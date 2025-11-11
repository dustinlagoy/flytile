# flytile

## setup

Run the server with docker-compose:

```yml
name: flytile
services:
    flytile:
        image: flytile
        volumes:
            - /opt/flytile:/cache
        env_file: .env
        environment:
            - ROCKET_ADDRESS=0.0.0.0
            - ROCKET_LOG_LEVEL=normal
            - FLYTILE_CACHE_DIR=/cache
        ports:
            - 8000:8000
        restart: always
        command: "server"
```

with a `.env` file that looks like:

```sh
FLYTILE_SRTM_PASSWORD=your-srtm-password
FLYTILE_SENTINEL_ID=your-copernicus-id
FLYTILE_SENTINEL_SECRET=your-copernicus-password
```

## acquiring credentials

For sentinel data:

- Make an account on https://dataspace.copernicus.eu.
- Create OAuth credentials in your user settings as described in the [docs](https://documentation.dataspace.copernicus.eu/APIs/SentinelHub/Overview/Authentication.html).
- Add these credentials to the `.env` file described [above](#setup).

For SRTM elevation data:

- Make an account on https://urs.earthdata.nasa.gov.
- Add these credentials to the `.env` file described [above](#setup).

## todo

- [x] logging
- [x] sentinel token generation
- [x] file cache manager
    - [x] can issue locks for updating
    - [x] can expire item generation on timeout
    - [x] can expire items on schedule
    - [x] can limit total size based on LRU or simple date
- [ ] selectable cloud coverage
- [ ] use cache for slope output tiles
- [ ] docs
- [ ] contours
- [ ] per-user urls
