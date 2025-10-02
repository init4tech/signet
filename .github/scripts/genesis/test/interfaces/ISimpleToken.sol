// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

import {IERC20Metadata} from "zenith/lib/openzeppelin-contracts/contracts/token/ERC20/extensions/IERC20Metadata.sol";

interface ISimpleToken is IERC20Metadata {
    function mint(address _to, uint256 _amount) external returns (bool);
}
