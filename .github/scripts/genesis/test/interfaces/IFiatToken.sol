// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

import {IERC20Metadata} from "zenith/lib/openzeppelin-contracts/contracts/token/ERC20/extensions/IERC20Metadata.sol";

interface IFiatToken is IERC20Metadata {
    function owner() external view returns (address);
    function pauser() external view returns (address);
    function blacklister() external view returns (address);
    function masterMinter() external view returns (address);

    function isMinter(address account) external view returns (bool);
    function minterAllowance(address minter) external view returns (uint256);
    function mint(address _to, uint256 _amount) external returns (bool);
}
